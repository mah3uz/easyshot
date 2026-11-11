use super::*;
use crate::free::Media;
use crate::free::fake::{Answer, Fake, Seen};
use es_model::share::ShareSource;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use time::PrimitiveDateTime;
use time::macros::format_description;
use uuid::Uuid;

const SECRET: &str = "s3cr3t/Key+never-shown";
const PUBLIC: &str = "https://cdn.example/pub";

type Rule = Box<dyn Fn(&Seen) -> Option<Answer> + Send + Sync>;

#[derive(Default)]
struct Store {
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    rule: Mutex<Option<Rule>>,
}

struct S3 {
    fake: Fake,
    store: Arc<Store>,
}

impl S3 {
    fn start() -> S3 {
        let store = Arc::new(Store::default());
        let shared = store.clone();
        let fake = Fake::start(move |seen| answer(&shared, seen));
        S3 { fake, store }
    }

    fn when(&self, rule: impl Fn(&Seen) -> Option<Answer> + Send + Sync + 'static) {
        *self.store.rule.lock().expect("rule") = Some(Box::new(rule));
    }

    fn objects(&self) -> BTreeMap<String, Vec<u8>> {
        self.store.objects.lock().expect("objects").clone()
    }

    fn config(&self, public: &str) -> BucketConfig {
        BucketConfig {
            endpoint: self.fake.base.clone(),
            bucket: "b".into(),
            region: "auto".into(),
            public_base_url: public.into(),
            prefix: "/shots".into(),
            keys: BucketKeys {
                access_key_id: "AKTEST".into(),
                secret_access_key: SECRET.into(),
            },
        }
    }

    fn bucket(&self) -> Bucket {
        Bucket::plain_http(&self.config(PUBLIC), "0.2.0").expect("config")
    }

    fn calls(&self) -> Vec<(String, String)> {
        let seen = self.fake.seen();
        seen.iter()
            .map(|s| (s.method.clone(), s.path.clone()))
            .collect()
    }
}

fn answer(store: &Store, seen: &Seen) -> Answer {
    if let Some(key) = seen.path.strip_prefix("/pub/") {
        let objects = store.objects.lock().expect("objects");
        return match objects.get(&format!("/b/{key}")) {
            Some(body) if seen.method == "GET" => Answer::Bytes(200, body.clone()),
            _ => Answer::Bytes(404, Vec::new()),
        };
    }
    if !signed_as_sent(seen) {
        return s3_error(403, "SignatureDoesNotMatch");
    }
    if let Some(answer) = store
        .rule
        .lock()
        .expect("rule")
        .as_ref()
        .and_then(|r| r(seen))
    {
        return answer;
    }
    let mut objects = store.objects.lock().expect("objects");
    match seen.method.as_str() {
        "PUT" => {
            assert_eq!(seen.body.len(), seen.body_len, "test bodies are kept whole");
            objects.insert(seen.path.clone(), seen.body.clone());
            Answer::Bytes(200, Vec::new())
        }
        "DELETE" => {
            objects.remove(&seen.path);
            Answer::Bytes(204, Vec::new())
        }
        _ => s3_error(405, "MethodNotAllowed"),
    }
}

fn s3_error(status: u16, code: &str) -> Answer {
    let xml = format!(
        "<?xml version=\"1.0\"?><Error><Code>{code}</Code><StringToSign>AWS4-HMAC-SHA256 {SECRET}</StringToSign></Error>"
    );
    Answer::Bytes(status, xml.into_bytes())
}

/// Re-signs what arrived; a header the client signed but sent differently fails here.
fn signed_as_sent(seen: &Seen) -> bool {
    let Some(auth) = seen.header("authorization") else {
        return false;
    };
    let Some(names) = auth
        .split(", ")
        .find_map(|part| part.strip_prefix("SignedHeaders="))
    else {
        return false;
    };
    let names: Vec<&str> = names.split(';').collect();
    for needed in ["host", "x-amz-content-sha256", "x-amz-date"] {
        assert!(names.contains(&needed), "{needed} is signed");
    }
    let headers: Vec<(&str, &str)> = names
        .iter()
        .filter(|n| **n != "x-amz-date")
        .map(|n| (*n, seen.header(n).unwrap_or("")))
        .collect();
    let date = seen.header("x-amz-date").unwrap_or("");
    let Ok(at) = PrimitiveDateTime::parse(
        date,
        format_description!("[year][month][day]T[hour][minute][second]Z"),
    ) else {
        return false;
    };
    let payload_hash = seen.header("x-amz-content-sha256").unwrap_or("");
    if payload_hash != sigv4::UNSIGNED_PAYLOAD && payload_hash != sigv4::sha256_hex(&seen.body) {
        return false;
    }
    let creds = sigv4::Credentials {
        access_key_id: "AKTEST",
        secret_access_key: SECRET,
        region: "auto",
        service: "s3",
    };
    let req = sigv4::Request {
        method: &seen.method,
        path: &seen.path,
        query: &[],
        headers: &headers,
        payload_hash,
    };
    sigv4::sign(&creds, &req, at.assume_utc()).is_ok_and(|s| s.authorization == auth)
}

fn file(dir: &TempDir, name: &str, len: u64) -> PathBuf {
    let path = dir.path().join(name);
    let mut f = File::create(&path).expect("create");
    let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    f.write_all(&bytes).expect("write");
    path
}

fn image(dir: &TempDir) -> ShareRequest {
    ShareRequest {
        media: Media {
            kind: ShareKind::Image,
            mime: "image/png".into(),
            path: file(dir, "a.png", 3000),
            width: 1920,
            height: 1080,
            duration_ms: None,
            poster: None,
        },
        title: Some("Login <bug>".into()),
        days: 7,
        source: ShareSource::Capture(Uuid::nil()),
    }
}

fn video(dir: &TempDir) -> ShareRequest {
    ShareRequest {
        media: Media {
            kind: ShareKind::Video,
            mime: "video/mp4".into(),
            path: file(dir, "v.mp4", 5000),
            width: 1280,
            height: 720,
            duration_ms: Some(4000),
            poster: Some(file(dir, "p.jpg", 700)),
        },
        title: None,
        days: 7,
        source: ShareSource::Recording("/v/a.easyshotrec".into()),
    }
}

fn share(
    bucket: &Bucket,
    req: &ShareRequest,
    cancel: &AtomicBool,
) -> (Result<ShareRecord, ShareError>, Vec<Event>) {
    let mut events = Vec::new();
    let result = bucket.share(req, &mut |e| events.push(e), cancel);
    (result, events)
}

fn fault(err: &ShareError) -> BucketFault {
    match err {
        ShareError::Bucket { fault, .. } => *fault,
        other => panic!("not a bucket error: {other:?}"),
    }
}

#[test]
fn an_image_share_writes_media_then_a_ready_page_with_every_header() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let req = image(&dir);
    let (record, events) = share(&s3.bucket(), &req, &AtomicBool::new(false));
    let record = record.expect("share");
    let id = record.id.clone();
    assert_eq!(id.len(), 22);
    assert!(valid_id(&id));
    let dir_key = format!("/b/shots/s/{id}");
    assert_eq!(
        s3.calls(),
        [
            ("PUT".into(), format!("{dir_key}/media.png")),
            ("PUT".into(), format!("{dir_key}/index.html")),
        ]
    );
    let seen = s3.fake.seen();
    let host = s3.fake.base.trim_start_matches("http://");
    let media = &seen[0];
    assert_eq!(media.header("host"), Some(host));
    assert_eq!(media.header("content-type"), Some("image/png"));
    assert_eq!(
        media.header("content-disposition"),
        Some(format!("inline; filename=\"easyshot-{id}.png\"").as_str())
    );
    assert_eq!(media.header("cache-control"), Some("public, max-age=3600"));
    assert_eq!(
        media.header("x-amz-content-sha256"),
        Some("UNSIGNED-PAYLOAD")
    );
    assert_eq!(media.header("content-length"), Some("3000"));
    assert_eq!(media.body, fs::read(&req.media.path).expect("read"));
    let index = &seen[1];
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(
        index.header("x-amz-content-sha256"),
        Some(sigv4::sha256_hex(&index.body).as_str()),
        "a body held in memory is signed with its hash"
    );
    for s in &seen {
        assert!(
            s.header("authorization")
                .is_some_and(|a| a.starts_with("AWS4-HMAC-SHA256 Credential=AKTEST/")
                    && a.contains("/auto/s3/aws4_request"))
        );
        assert!(!s.headers.iter().any(|(_, v)| v.contains(SECRET)));
    }

    let page = String::from_utf8(index.body.clone()).expect("utf-8");
    let media_url = format!("{PUBLIC}/shots/s/{id}/media.png");
    let page_url = format!("{PUBLIC}/shots/s/{id}/index.html");
    assert!(page.contains(&format!(
        "<img class=\"media\" data-fit src=\"{media_url}\""
    )));
    assert!(page.contains(&page_url));
    assert!(page.contains("Login &lt;bug&gt;"));
    assert!(page.contains(&format!("easyshot-{id}.png")));
    assert!(page.contains("https://easyshot.site"));
    assert!(!page.contains("<video"), "an image page has no video");
    assert!(!page.contains("{{") && !page.contains("<!--if:"));

    assert_eq!(
        events.first(),
        Some(&Event::Reserved {
            page_url: page_url.clone(),
            media_url: media_url.clone(),
            expires_at: None,
        })
    );
    assert_eq!(
        events.last(),
        Some(&Event::Progress {
            sent: 3000,
            total: 3000
        })
    );
    assert_eq!(record.target, ShareTarget::Bucket { label: "b".into() });
    assert_eq!((record.page_url, record.media_url), (page_url, media_url));
    assert_eq!(record.poster_url, None);
    assert_eq!((record.days, record.expires_at), (None, None));
    assert_eq!(record.delete, ShareDelete::Bucket);
    assert_eq!(record.state, ShareState::Ready);
}

#[test]
fn a_video_share_writes_its_poster_and_a_video_page() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let req = video(&dir);
    let (record, events) = share(&s3.bucket(), &req, &AtomicBool::new(false));
    let record = record.expect("share");
    let id = &record.id;
    let keys: Vec<String> = s3.objects().into_keys().collect();
    assert_eq!(
        keys,
        [
            format!("/b/shots/s/{id}/index.html"),
            format!("/b/shots/s/{id}/media.mp4"),
            format!("/b/shots/s/{id}/poster.jpg"),
        ]
    );
    let poster = &s3.fake.seen()[1];
    assert_eq!(poster.header("content-type"), Some("image/jpeg"));
    assert_eq!(
        poster.header("x-amz-content-sha256"),
        Some("UNSIGNED-PAYLOAD")
    );
    let poster_url = format!("{PUBLIC}/shots/s/{id}/poster.jpg");
    assert_eq!(record.poster_url.as_deref(), Some(poster_url.as_str()));
    let page = String::from_utf8(s3.objects()[&keys[0]].clone()).expect("utf-8");
    assert!(page.contains(&format!("poster=\"{poster_url}\"")));
    assert!(page.contains("<video"));
    assert!(
        page.contains("<title>Shared with easyshot</title>"),
        "an untitled share reads as the service names it"
    );
    assert_eq!(
        events.last(),
        Some(&Event::Progress {
            sent: 5700,
            total: 5700
        })
    );
}

#[test]
fn a_failed_second_put_deletes_the_media_already_written() {
    let s3 = S3::start();
    s3.when(|seen| {
        (seen.method == "PUT" && seen.path.ends_with("/poster.jpg"))
            .then(|| s3_error(500, "InternalError"))
    });
    let dir = TempDir::new().expect("tmp");
    let (result, _) = share(&s3.bucket(), &video(&dir), &AtomicBool::new(false));
    let err = result.expect_err("poster refused");
    assert_eq!(fault(&err), BucketFault::Bucket);
    assert!(s3.objects().is_empty(), "nothing is left in the bucket");
    let methods: Vec<String> = s3.calls().into_iter().map(|(m, _)| m).collect();
    assert_eq!(methods, ["PUT", "PUT", "DELETE", "DELETE"]);
}

#[test]
fn cancel_mid_upload_deletes_what_was_written_and_never_writes_the_page() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let cancel = AtomicBool::new(false);
    let mut on_event = |e: Event| {
        if matches!(e, Event::Progress { sent, .. } if sent >= 5000) {
            cancel.store(true, Ordering::Relaxed);
        }
    };
    let err = s3
        .bucket()
        .share(&video(&dir), &mut on_event, &cancel)
        .expect_err("cancelled");
    assert!(matches!(err, ShareError::Cancelled), "{err:?}");
    assert!(s3.objects().is_empty());
    assert!(!s3.calls().iter().any(|(_, p)| p.ends_with("index.html")));
    assert!(
        s3.calls()
            .iter()
            .any(|(m, p)| m == "DELETE" && p.ends_with("media.mp4"))
    );
}

#[test]
fn refused_keys_are_named_without_echoing_what_was_signed() {
    let s3 = S3::start();
    s3.when(|_| Some(s3_error(403, "InvalidAccessKeyId")));
    let dir = TempDir::new().expect("tmp");
    let (result, _) = share(&s3.bucket(), &image(&dir), &AtomicBool::new(false));
    let err = result.expect_err("refused");
    assert_eq!(fault(&err), BucketFault::Credentials);
    let shown = format!("{err} {err:?}");
    assert!(shown.contains("InvalidAccessKeyId"));
    assert!(!shown.contains(SECRET) && !shown.contains("AWS4-HMAC"));
}

#[test]
fn delete_removes_page_media_and_poster_and_gone_counts_as_deleted() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let bucket = s3.bucket();
    let (record, _) = share(&bucket, &video(&dir), &AtomicBool::new(false));
    let record = record.expect("share");
    bucket.delete(&record).expect("delete");
    assert!(s3.objects().is_empty());
    s3.when(|seen| (seen.method == "DELETE").then(|| s3_error(404, "NoSuchKey")));
    bucket.delete(&record).expect("already gone");
}

#[test]
fn delete_refuses_a_share_made_under_other_settings() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let (record, _) = share(&s3.bucket(), &image(&dir), &AtomicBool::new(false));
    let record = record.expect("share");
    let mut cfg = s3.config(PUBLIC);
    cfg.prefix = "elsewhere".into();
    let moved = Bucket::plain_http(&cfg, "0.2.0").expect("config");
    let err = moved.delete(&record).expect_err("other prefix");
    assert_eq!(fault(&err), BucketFault::Bucket);
    assert!(!s3.calls().iter().any(|(m, _)| m == "DELETE"));
    assert_eq!(s3.objects().len(), 2);
}

#[test]
fn files_over_five_gib_and_unknown_types_are_refused_before_any_request() {
    let s3 = S3::start();
    let dir = TempDir::new().expect("tmp");
    let req = image(&dir);
    File::options()
        .write(true)
        .open(&req.media.path)
        .expect("open")
        .set_len(MAX_PUT + 1)
        .expect("sparse");
    let (result, _) = share(&s3.bucket(), &req, &AtomicBool::new(false));
    assert!(matches!(
        result,
        Err(ShareError::Service {
            code: Code::TooLarge,
            ..
        })
    ));
    let mut req = image(&dir);
    req.media.mime = "text/html".into();
    let (result, _) = share(&s3.bucket(), &req, &AtomicBool::new(false));
    assert!(matches!(
        result,
        Err(ShareError::Service {
            code: Code::UnsupportedType,
            ..
        })
    ));
    assert!(s3.calls().is_empty());
}

#[test]
fn test_connection_writes_reads_back_publicly_and_deletes() {
    let s3 = S3::start();
    let bucket =
        Bucket::plain_http(&s3.config(&format!("{}/pub", s3.fake.base)), "0.2.0").expect("config");
    bucket.test_connection().expect("works");
    let calls = s3.calls();
    let methods: Vec<&str> = calls.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(methods, ["PUT", "GET", "DELETE"]);
    assert!(calls[0].1.starts_with("/b/shots/easyshot-test-"));
    assert_eq!(calls[1].1, calls[0].1.replacen("/b/", "/pub/", 1));
    assert!(
        s3.fake.seen()[1].header("authorization").is_none(),
        "the public read is anonymous"
    );
    assert!(s3.objects().is_empty());
}

#[test]
fn test_connection_names_the_step_that_failed() {
    let s3 = S3::start();
    let base = s3.fake.base.clone();
    let public_ok = format!("{base}/pub");
    let check = |public: &str, want: BucketFault| {
        let bucket = Bucket::plain_http(&s3.config(public), "0.2.0").expect("config");
        let err = bucket.test_connection().expect_err("fails");
        assert_eq!(fault(&err), want, "{err}");
        assert!(s3.objects().is_empty(), "the test object is always removed");
    };
    check(&format!("{base}/elsewhere"), BucketFault::PublicUrl);
    check(PUBLIC, BucketFault::PublicUrl);
    s3.when(|seen| (seen.method == "PUT").then(|| Answer::Bytes(200, Vec::new())));
    check(&public_ok, BucketFault::PublicUrl);
    s3.when(|_| Some(s3_error(403, "SignatureDoesNotMatch")));
    check(&public_ok, BucketFault::Credentials);
    s3.when(|_| Some(s3_error(404, "NoSuchBucket")));
    check(&public_ok, BucketFault::Bucket);

    let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut cfg = s3.config(&public_ok);
    cfg.endpoint = format!("http://{}", closed.local_addr().expect("addr"));
    drop(closed);
    let err = Bucket::plain_http(&cfg, "0.2.0")
        .expect("config")
        .test_connection()
        .expect_err("nothing listens");
    assert_eq!(fault(&err), BucketFault::Network);
}

#[test]
fn the_config_is_checked_and_the_prefix_normalised() {
    let cfg = |endpoint: &str, public: &str, region: &str, bucket: &str| BucketConfig {
        endpoint: endpoint.into(),
        bucket: bucket.into(),
        region: region.into(),
        public_base_url: public.into(),
        prefix: String::new(),
        keys: BucketKeys {
            access_key_id: "AK".into(),
            secret_access_key: "SK".into(),
        },
    };
    let r2 = "https://acct.r2.cloudflarestorage.com";
    let good = Bucket::new(&cfg(r2, "https://pub.example/", "auto", "shots"), "1").expect("R2");
    assert_eq!(good.host, "acct.r2.cloudflarestorage.com");
    assert_eq!(good.public_base_url, "https://pub.example");
    assert_eq!(good.prefix, "");
    let port =
        Bucket::new(&cfg("https://s3.example:9000", PUBLIC, "auto", "b"), "1").expect("port");
    assert_eq!(port.host, "s3.example:9000");
    let default_port = Bucket::new(&cfg("https://s3.example:443/", PUBLIC, "auto", "b"), "1");
    assert_eq!(default_port.expect("443").host, "s3.example");

    let refused = [
        (
            cfg("http://s3.example", PUBLIC, "auto", "b"),
            ConfigError::Endpoint,
        ),
        (
            cfg("https://s3.example/b", PUBLIC, "auto", "b"),
            ConfigError::Endpoint,
        ),
        (
            cfg("https://u:p@s3.example", PUBLIC, "auto", "b"),
            ConfigError::Endpoint,
        ),
        (
            cfg(r2, "http://pub.example", "auto", "b"),
            ConfigError::PublicBaseUrl,
        ),
        (
            cfg(r2, "pub.example", "auto", "b"),
            ConfigError::PublicBaseUrl,
        ),
        (cfg(r2, PUBLIC, " ", "b"), ConfigError::Region),
        (cfg(r2, PUBLIC, "auto", ""), ConfigError::BucketName),
        (cfg(r2, PUBLIC, "auto", "a/b"), ConfigError::BucketName),
    ];
    for (c, want) in refused {
        assert_eq!(Bucket::new(&c, "1").err(), Some(want), "{c:?}");
    }
    let mut no_secret = cfg(r2, PUBLIC, "auto", "b");
    no_secret.keys.secret_access_key.clear();
    assert_eq!(Bucket::new(&no_secret, "1").err(), Some(ConfigError::Keys));

    for (given, want) in [("shots", "shots/"), ("/a/b/", "a/b/"), (" ", "")] {
        let mut c = cfg(r2, PUBLIC, "auto", "b");
        c.prefix = given.into();
        assert_eq!(Bucket::new(&c, "1").expect("prefix").prefix, want);
    }
}

#[test]
fn ids_are_22_random_url_safe_characters() {
    let a = random_id().expect("id");
    let b = random_id().expect("id");
    assert!(valid_id(&a) && valid_id(&b));
    assert_ne!(a, b);
}
