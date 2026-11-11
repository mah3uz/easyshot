use super::fake::{Answer, Fake, Seen, contract_answer, error, exchange};
use super::*;
use serde_json::Value;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, MutexGuard};
use tempfile::TempDir;
use uuid::Uuid;

const VERSION: &str = "0.2.0";
const IMAGE_ID: &str = "7Kp2mX9qR4tY8wZ1aB3cD5";
const VIDEO_ID: &str = "3Qw8eR2tY6uI0oP4aS1dF7";
const PART: u64 = 26214400;

#[derive(Default)]
struct Log {
    token: Option<String>,
    saved: Vec<String>,
    clears: usize,
}

#[derive(Default, Clone)]
struct Tokens(Arc<Mutex<Log>>);

impl Tokens {
    fn holding(token: &str) -> Self {
        let tokens = Tokens::default();
        tokens.log().token = Some(token.into());
        tokens
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        self.0.lock().expect("tokens")
    }
}

impl InstallTokens for Tokens {
    fn load(&self) -> Option<String> {
        self.log().token.clone()
    }
    fn save(&self, token: &str) {
        let mut log = self.log();
        log.token = Some(token.into());
        log.saved.push(token.into());
    }
    fn clear(&self) {
        let mut log = self.log();
        log.token = None;
        log.clears += 1;
    }
}

fn file(dir: &TempDir, name: &str, magic: &[u8], len: u64) -> PathBuf {
    let path = dir.path().join(name);
    let mut f = File::create(&path).expect("create");
    f.write_all(magic).expect("magic");
    f.set_len(len).expect("size");
    path
}

fn image_request(dir: &TempDir) -> ShareRequest {
    ShareRequest {
        media: Media {
            kind: ShareKind::Image,
            mime: "image/png".into(),
            path: file(dir, "a.png", b"\x89PNG", 1843210),
            width: 1920,
            height: 1080,
            duration_ms: None,
            poster: None,
        },
        title: Some("Login bug".into()),
        days: 7,
        source: ShareSource::Capture(Uuid::nil()),
    }
}

fn video_request(dir: &TempDir) -> ShareRequest {
    ShareRequest {
        media: Media {
            kind: ShareKind::Video,
            mime: "video/mp4".into(),
            path: file(dir, "a.mp4", b"\0\0\0\x18ftyp", 62914560),
            width: 1920,
            height: 1080,
            duration_ms: Some(245000),
            poster: Some(file(dir, "poster.jpg", b"\xff\xd8\xff", 81234)),
        },
        title: None,
        days: 3,
        source: ShareSource::Recording("/v/a.easyshotrec".into()),
    }
}

fn run(
    fake: &Fake,
    tokens: &Tokens,
    req: &ShareRequest,
    cancel_after: Option<u64>,
) -> (Result<ShareRecord, ShareError>, Vec<Event>) {
    let free = Free::plain_http(&fake.base, VERSION, tokens.clone());
    let cancel = AtomicBool::new(false);
    let mut events = Vec::new();
    let mut on_event = |event: Event| {
        if let (Event::Progress { sent, .. }, Some(after)) = (&event, cancel_after)
            && *sent > after
        {
            cancel.store(true, Ordering::Relaxed);
        }
        events.push(event);
    };
    let result = free.share(req, &mut on_event, &cancel);
    let seen = fake.seen();
    for s in &seen {
        assert_eq!(s.header("X-Easyshot-Version"), Some(VERSION), "{s:?}");
        assert_eq!(s.header("User-Agent"), Some("easyshot/0.2.0"), "{s:?}");
        assert_eq!(
            s.header("Origin"),
            None,
            "the service refuses any request with an Origin"
        );
    }
    (result, events)
}

fn calls(fake: &Fake) -> Vec<String> {
    fake.seen()
        .iter()
        .map(|s| format!("{} {}", s.method, s.path))
        .collect()
}

fn assert_exchange(seen: &Seen, name: &str) {
    let req = &exchange(name)["request"];
    assert!(
        seen.is(
            req["method"].as_str().expect("method"),
            req["path"].as_str().expect("path")
        ),
        "{name}: {} {}",
        seen.method,
        seen.path
    );
    for (header, value) in req["headers"].as_object().expect("headers") {
        assert_eq!(seen.header(header), value.as_str(), "{name}: {header}");
    }
    if let Some(body) = req.get("body") {
        assert_eq!(&seen.json(), body, "{name}: body");
    }
}

#[test]
fn an_image_share_installs_creates_uploads_and_completes() {
    let dir = TempDir::new().expect("dir");
    let fake = Fake::contract();
    let tokens = Tokens::default();
    let (result, events) = run(&fake, &tokens, &image_request(&dir), None);
    let record = result.expect("shared");

    let seen = fake.seen();
    assert_eq!(
        calls(&fake),
        [
            "GET /v1/challenge".to_owned(),
            "POST /v1/install".into(),
            "POST /v1/shares".into(),
            format!("PUT /v1/shares/{IMAGE_ID}/parts/1"),
            format!("POST /v1/shares/{IMAGE_ID}/complete"),
        ]
    );
    assert_exchange(&seen[0], "challenge");
    assert_exchange(&seen[1], "install");
    assert_exchange(&seen[2], "create_image");
    assert_eq!(seen[3].header("Authorization"), Some("Bearer ut_example"));
    assert_eq!(seen[3].body_len, 1843210);
    assert_eq!(&seen[3].body[..4], b"\x89PNG");
    assert_eq!(tokens.log().saved, ["it_example"]);

    let created = &exchange("create_image")["response"]["body"];
    assert_eq!(record.id, IMAGE_ID);
    assert_eq!(record.page_url, created["page_url"].as_str().expect("page"));
    assert_eq!(record.state, ShareState::Ready);
    assert_eq!(record.delete, ShareDelete::FreeToken("dt_example".into()));
    assert_eq!(
        (record.days, record.title.as_deref()),
        (Some(7), Some("Login bug"))
    );
    assert_eq!(
        record.expires_at,
        Some(parse_time("2026-11-18T10:00:00Z").expect("time"))
    );

    assert!(
        matches!(&events[0], Event::Reserved { page_url, .. } if page_url == created["page_url"].as_str().expect("page")),
        "the link is handed over before any byte is sent"
    );
    assert_eq!(
        events.last(),
        Some(&Event::Progress {
            sent: 1843210,
            total: 1843210
        })
    );
}

#[test]
fn a_video_share_sends_three_parts_then_the_poster_then_completes() {
    let dir = TempDir::new().expect("dir");
    let fake = Fake::contract();
    let tokens = Tokens::holding("it_example");
    let (result, events) = run(&fake, &tokens, &video_request(&dir), None);
    let record = result.expect("shared");

    let seen = fake.seen();
    let parts = |n| format!("PUT /v1/shares/{VIDEO_ID}/parts/{n}");
    assert_eq!(
        calls(&fake),
        [
            "POST /v1/shares".to_owned(),
            parts(1),
            parts(2),
            parts(3),
            format!("PUT /v1/shares/{VIDEO_ID}/poster"),
            format!("POST /v1/shares/{VIDEO_ID}/complete"),
        ]
    );
    assert_exchange(&seen[0], "create_video");
    assert_eq!(
        (seen[1].body_len as u64, seen[2].body_len as u64),
        (PART, PART)
    );
    assert_exchange(&seen[3], "part_last");
    assert_exchange(&seen[4], "poster");
    assert_exchange(&seen[5], "complete");
    assert_eq!(&seen[4].body[..3], b"\xff\xd8\xff");
    assert!(tokens.log().saved.is_empty(), "a stored token is reused");

    assert_eq!(record.kind, ShareKind::Video);
    assert_eq!((record.days, record.title.as_deref()), (Some(3), None));
    assert_eq!(
        record.poster_url.as_deref(),
        Some("https://m.easyshot.site/s/3d/3Qw8eR2tY6uI0oP4aS1dF7/poster.jpg")
    );
    let progress: Vec<(u64, u64)> = events
        .iter()
        .filter_map(|e| match e {
            Event::Progress { sent, total } => Some((*sent, *total)),
            Event::Reserved { .. } => None,
        })
        .collect();
    assert!(progress.windows(2).all(|w| w[0].0 <= w[1].0));
    assert_eq!(progress.last(), Some(&(62914560 + 81234, 62914560 + 81234)));
}

fn failing_part(failures: usize) -> (Fake, Arc<AtomicUsize>) {
    let tries = Arc::new(AtomicUsize::new(0));
    let count = tries.clone();
    let fake = Fake::start(move |seen| {
        if seen.path.ends_with("/parts/2") {
            match count.fetch_add(1, Ordering::SeqCst) {
                n if n >= failures => {}
                0 => return Answer::Drop,
                _ => return Answer::Reply(503, None),
            }
        }
        contract_answer(seen)
    });
    (fake, tries)
}

#[test]
fn a_part_that_fails_twice_is_retried_and_the_share_completes() {
    let dir = TempDir::new().expect("dir");
    let (fake, tries) = failing_part(2);
    let (result, _) = run(
        &fake,
        &Tokens::holding("it_example"),
        &video_request(&dir),
        None,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(tries.load(Ordering::SeqCst), 3);
    let seen = fake.seen();
    assert!(seen.iter().any(|s| s.path.ends_with("/complete")));
    assert!(!seen.iter().any(|s| s.method == "DELETE"));
}

#[test]
fn a_part_that_keeps_failing_gives_up_after_three_retries_and_deletes_the_share() {
    let dir = TempDir::new().expect("dir");
    let (fake, tries) = failing_part(usize::MAX);
    let (result, _) = run(
        &fake,
        &Tokens::holding("it_example"),
        &video_request(&dir),
        None,
    );
    assert!(matches!(result, Err(ShareError::Protocol(_))), "{result:?}");
    assert_eq!(tries.load(Ordering::SeqCst), 4);
    let seen = fake.seen();
    assert_exchange(seen.last().expect("delete"), "delete");
    assert!(!seen.iter().any(|s| s.path.ends_with("/parts/3")));
}

#[test]
fn cancel_during_part_two_deletes_the_share_and_sends_nothing_more() {
    let dir = TempDir::new().expect("dir");
    let fake = Fake::contract();
    let (result, _) = run(
        &fake,
        &Tokens::holding("it_example"),
        &video_request(&dir),
        Some(PART + 1),
    );
    assert!(matches!(result, Err(ShareError::Cancelled)), "{result:?}");
    let seen = fake.seen();
    assert_exchange(seen.last().expect("delete"), "delete");
    assert!(
        !seen.iter().any(|s| s.path.ends_with("/parts/3")
            || s.path.ends_with("/complete")
            || s.path.ends_with("/parts/2")),
        "part 2 never arrives whole, and nothing follows it: {:?}",
        calls(&fake)
    );
}

fn refusing(token: &'static str) -> Fake {
    Fake::start(move |seen| {
        let auth = format!("Bearer {token}");
        if seen.is("POST", "/v1/shares")
            && (token == "*" || seen.header("Authorization") == Some(&auth))
        {
            return error(401, "unauthorized");
        }
        contract_answer(seen)
    })
}

#[test]
fn a_stale_install_token_is_replaced_once_on_401() {
    let dir = TempDir::new().expect("dir");
    let fake = refusing("it_stale");
    let tokens = Tokens::holding("it_stale");
    let (result, _) = run(&fake, &tokens, &image_request(&dir), None);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        &calls(&fake)[..4],
        [
            "POST /v1/shares",
            "GET /v1/challenge",
            "POST /v1/install",
            "POST /v1/shares"
        ]
    );
    assert_eq!(
        fake.seen()[3].header("Authorization"),
        Some("Bearer it_example")
    );
    let log = tokens.log();
    assert_eq!(
        (log.clears, log.saved.as_slice()),
        (1, &["it_example".to_owned()][..])
    );
}

#[test]
fn a_401_after_reinstalling_is_returned_not_retried_forever() {
    let dir = TempDir::new().expect("dir");
    let fake = refusing("*");
    let (result, _) = run(
        &fake,
        &Tokens::holding("it_stale"),
        &image_request(&dir),
        None,
    );
    assert_eq!(
        result.as_ref().err().and_then(ShareError::code),
        Some(Code::Unauthorized)
    );
    let calls = calls(&fake);
    assert_eq!(calls.iter().filter(|c| *c == "POST /v1/shares").count(), 2);
    assert_eq!(calls.iter().filter(|c| *c == "POST /v1/install").count(), 1);
}

#[test]
fn every_service_error_code_maps_and_its_message_stays_out() {
    let doc: Value = serde_json::from_str(include_str!(
        "../../../../services/share/contract/errors.json"
    ))
    .expect("errors.json parses");
    let codes = doc["codes"].as_object().expect("codes");
    assert_eq!(codes.len(), 13);
    let examples = doc["examples"].as_array().expect("examples");
    let dir = TempDir::new().expect("dir");
    for (name, status) in codes {
        let retry = examples
            .iter()
            .find(|e| e["body"]["error"]["code"] == name.as_str())
            .and_then(|e| e["body"]["error"]["retry_after_s"].as_u64());
        let body = serde_json::json!({ "error": {
            "code": name, "message": "internal detail", "retry_after_s": retry } });
        let status = status.as_u64().expect("status") as u16;
        let fake = Fake::start(move |seen| match seen.is("POST", "/v1/shares") {
            true => Answer::Reply(status, Some(body.clone())),
            false => contract_answer(seen),
        });
        let (result, _) = run(
            &fake,
            &Tokens::holding("it_example"),
            &image_request(&dir),
            None,
        );
        let err = result.expect_err(name);
        let ShareError::Service {
            code,
            retry_after_s,
        } = &err
        else {
            panic!("{name}: {err:?}");
        };
        assert_eq!(serde_json::to_value(code).expect("code"), name.as_str());
        assert_eq!(*retry_after_s, retry, "{name}");
        assert!(!err.to_string().contains("internal detail"), "{name}");
        assert!(
            !calls(&fake).iter().any(|c| c.starts_with("DELETE")),
            "nothing was created"
        );
    }
}

#[test]
fn delete_sends_the_delete_token_and_a_gone_share_counts_as_deleted() {
    let dir = TempDir::new().expect("dir");
    let fake = Fake::contract();
    let (record, _) = run(
        &fake,
        &Tokens::holding("it_example"),
        &video_request(&dir),
        None,
    );
    let record = record.expect("shared");
    let free = Free::plain_http(&fake.base, VERSION, Tokens::default());
    free.delete(&record).expect("deleted");
    assert_exchange(fake.seen().last().expect("delete"), "delete");

    let gone = Fake::start(|_| error(404, "not_found"));
    let free = Free::plain_http(&gone.base, VERSION, Tokens::default());
    free.delete(&record)
        .expect("a gone share is already deleted");
}

#[test]
fn a_share_id_outside_the_protocol_never_reaches_a_request_path() {
    let dir = TempDir::new().expect("dir");
    let fake = Fake::start(|seen| match seen.is("POST", "/v1/shares") {
        true => {
            let mut body = exchange("create_image")["response"]["body"].clone();
            body["id"] = "7/../../install?x=aaaaa".into();
            Answer::Reply(201, Some(body))
        }
        false => contract_answer(seen),
    });
    let (result, events) = run(
        &fake,
        &Tokens::holding("it_example"),
        &image_request(&dir),
        None,
    );
    assert!(matches!(result, Err(ShareError::Protocol(_))), "{result:?}");
    assert!(events.is_empty());
    assert_eq!(calls(&fake), ["POST /v1/shares"]);
}

#[test]
fn limits_reads_the_service_and_the_release_client_refuses_plain_http() {
    let fake = Fake::contract();
    let free = Free::plain_http(&fake.base, VERSION, Tokens::holding("it_example"));
    let limits = free.limits().expect("limits");
    assert_eq!((limits.shares_left, limits.part_bytes), (Some(29), PART));
    assert_exchange(&fake.seen()[0], "limits");

    let release = Free::new(&fake.base, VERSION, Tokens::default());
    assert!(matches!(release.limits(), Err(ShareError::Network(_))));
    assert_eq!(
        fake.seen().len(),
        1,
        "no plain-http request left the release client"
    );
}
