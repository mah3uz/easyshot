use super::*;
use crate::free::Media;
use es_model::share::ShareSource;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use uuid::Uuid;

const USER: &str = "easyshot-test";
const PASSWORD: &str = "easyshot-test-secret";
const NAME: &str = "shots";

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
}

fn admin(bucket: &Bucket, path: &str, query: &str, body: &[u8]) -> u16 {
    let hash = sigv4::sha256_hex(body);
    let headers = [
        ("host", bucket.host.as_str()),
        ("x-amz-content-sha256", hash.as_str()),
    ];
    let creds = sigv4::Credentials {
        access_key_id: USER,
        secret_access_key: PASSWORD,
        region: "us-east-1",
        service: "s3",
    };
    let pairs: Vec<(&str, &str)> = match query {
        "" => Vec::new(),
        q => vec![(q, "")],
    };
    let req = sigv4::Request {
        method: "PUT",
        path,
        query: &pairs,
        headers: &headers,
        payload_hash: &hash,
    };
    let signed = sigv4::sign(&creds, &req, OffsetDateTime::now_utc()).expect("sign");
    let mut sent = headers.to_vec();
    sent.push(("x-amz-date", &signed.amz_date));
    sent.push(("authorization", &signed.authorization));
    let url = match query {
        "" => format!("{}{path}", bucket.endpoint),
        q => format!("{}{path}?{q}=", bucket.endpoint),
    };
    let len = body.len() as u64;
    let reply = bucket
        .http
        .send(
            "PUT",
            &url,
            &sent,
            Some((len, &mut &body[..] as &mut dyn Read)),
        )
        .expect("admin request");
    reply.status
}

#[test]
fn round_trip_against_a_throwaway_minio() {
    if !on_path("minio") {
        eprintln!("skipped: no minio binary on PATH");
        return;
    }
    let data = TempDir::new().expect("tmp");
    let port = free_port();
    let child = Command::new("minio")
        .args(["server", "--quiet", "--address"])
        .arg(format!("127.0.0.1:{port}"))
        .arg("--console-address")
        .arg(format!("127.0.0.1:{}", free_port()))
        .arg(data.path().join("data"))
        .env("MINIO_ROOT_USER", USER)
        .env("MINIO_ROOT_PASSWORD", PASSWORD)
        .env("MINIO_BROWSER", "off")
        .env("HOME", data.path())
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start minio");
    let _server = Server(child);
    let local = format!("http://127.0.0.1:{port}");
    let probe = Http::new("", "test", false);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !probe
        .send("GET", &format!("{local}/minio/health/live"), &[], None)
        .is_ok_and(|r| r.status == 200)
    {
        assert!(Instant::now() < deadline, "minio did not start");
        thread::sleep(Duration::from_millis(100));
    }

    let cfg = |public: &str| BucketConfig {
        endpoint: local.clone(),
        bucket: NAME.into(),
        region: "us-east-1".into(),
        public_base_url: public.into(),
        prefix: "team/".into(),
        keys: BucketKeys {
            access_key_id: USER.into(),
            secret_access_key: PASSWORD.into(),
        },
    };
    let public = format!("{local}/{NAME}");
    let bucket = Bucket::plain_http(&cfg(&public), "test").expect("config");
    assert_eq!(admin(&bucket, &format!("/{NAME}"), "", b""), 200);
    let policy = format!(
        r#"{{"Version":"2012-10-17","Statement":[{{"Effect":"Allow","Principal":{{"AWS":["*"]}},"Action":["s3:GetObject"],"Resource":["arn:aws:s3:::{NAME}/*"]}}]}}"#
    );
    assert_eq!(
        admin(&bucket, &format!("/{NAME}"), "policy", policy.as_bytes()),
        204
    );
    bucket.test_connection().expect("connection test");

    let dir = TempDir::new().expect("tmp");
    let path = dir.path().join("a.png");
    let bytes: Vec<u8> = b"\x89PNG".iter().copied().chain(0..=255).collect();
    fs::write(&path, &bytes).expect("write");
    let req = ShareRequest {
        media: Media {
            kind: ShareKind::Image,
            mime: "image/png".into(),
            path,
            width: 16,
            height: 16,
            duration_ms: None,
            poster: None,
        },
        title: Some("MinIO round trip".into()),
        days: 7,
        source: ShareSource::Capture(Uuid::nil()),
    };
    let https_public = "https://minio.invalid/shots";
    let sharing = Bucket::plain_http(&cfg(https_public), "test").expect("config");
    let record = sharing
        .share(&req, &mut |_| {}, &AtomicBool::new(false))
        .expect("share");
    let fetch = |url: &str| {
        let url = url.replacen(https_public, &public, 1);
        probe.send("GET", &url, &[], None).expect("fetch")
    };
    let media = fetch(&record.media_url);
    assert_eq!((media.status, media.body()), (200, &bytes[..]));
    let page = fetch(&record.page_url);
    assert_eq!(page.status, 200);
    assert!(String::from_utf8_lossy(page.body()).contains("MinIO round trip"));

    sharing.delete(&record).expect("delete");
    assert_ne!(fetch(&record.media_url).status, 200);
    assert_ne!(fetch(&record.page_url).status, 200);
}
