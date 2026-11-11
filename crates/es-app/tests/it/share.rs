#![allow(clippy::unwrap_used)]
use es_app::errors::Result;
use es_app::history::History;
use es_app::platform_ui::ClipboardSink;
use es_app::services::Strings;
use es_app::settings_model::SettingsModel;
use es_app::share::{
    ShareDeleted, ShareFailure, ShareJobEvent, ShareJobKind, ShareStart, Shares, Target, Targets,
};
use es_model::platform::ClipItem;
use es_model::{
    CaptureKind, HistoryEntry, ShareCfg, ShareDelete, ShareKind, ShareMode, ShareRecord,
    ShareSource, ShareState, ShareTarget,
};
use es_share::{Code, Event, Media, ShareError, ShareRequest};
use es_store::{ConfigFile, Paths};
use gpui::{App, AppContext, Entity, Task, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use time::OffsetDateTime;
use uuid::Uuid;

const PAGE: &str = "https://share.example/s/abc123";

type Outcome =
    Box<dyn Fn(&ShareRequest) -> std::result::Result<ShareRecord, ShareError> + Send + Sync>;
type DeleteOutcome = Box<dyn Fn(&ShareRecord) -> std::result::Result<(), ShareError> + Send + Sync>;

struct Fake {
    opened: AtomicUsize,
    outcome: Outcome,
    delete: DeleteOutcome,
    deleted: Mutex<Vec<String>>,
    gate: Mutex<Option<mpsc::Receiver<()>>>,
}

impl Fake {
    fn new(outcome: Outcome) -> Arc<Self> {
        Arc::new(Self {
            opened: AtomicUsize::new(0),
            outcome,
            delete: Box::new(|_| Ok(())),
            deleted: Mutex::default(),
            gate: Mutex::default(),
        })
    }

    fn deleting(delete: DeleteOutcome) -> Arc<Self> {
        Arc::new(Self {
            delete,
            ..Arc::into_inner(Self::new(Box::new(|r| Ok(made(r))))).unwrap()
        })
    }
}

struct FakeTarget(Arc<Fake>);

impl Target for FakeTarget {
    fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        _cancel: &AtomicBool,
    ) -> std::result::Result<ShareRecord, ShareError> {
        on_event(Event::Reserved {
            page_url: PAGE.into(),
            media_url: format!("{PAGE}.png"),
            expires_at: None,
        });
        on_event(Event::Progress { sent: 5, total: 10 });
        if let Some(gate) = self.0.gate.lock().unwrap().take() {
            let _ = gate.recv();
        }
        (self.0.outcome)(req)
    }

    fn delete(&self, record: &ShareRecord) -> std::result::Result<(), ShareError> {
        self.0.deleted.lock().unwrap().push(record.id.clone());
        (self.0.delete)(record)
    }
}

struct FakeTargets(Arc<Fake>);

impl Targets for FakeTargets {
    fn open(
        &self,
        _mode: ShareMode,
        _cfg: &ShareCfg,
    ) -> std::result::Result<Box<dyn Target>, ShareFailure> {
        self.0.opened.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeTarget(self.0.clone())))
    }
}

#[derive(Default)]
struct Clip(RefCell<Vec<ClipItem>>);

impl ClipboardSink for Clip {
    fn copy(&self, items: Vec<ClipItem>, _: &mut App) -> Task<Result<()>> {
        self.0.borrow_mut().extend(items);
        Task::ready(Ok(()))
    }
}

fn made(req: &ShareRequest) -> ShareRecord {
    let now = OffsetDateTime::now_utc();
    record(
        "abc123",
        req.source.clone(),
        Some(now + time::Duration::days(7)),
    )
}

fn record(id: &str, source: ShareSource, expires_at: Option<OffsetDateTime>) -> ShareRecord {
    ShareRecord {
        id: id.into(),
        target: ShareTarget::Free,
        kind: ShareKind::Image,
        page_url: format!("https://share.example/s/{id}"),
        media_url: format!("https://share.example/m/{id}.png"),
        poster_url: None,
        title: None,
        days: Some(7),
        created_at: OffsetDateTime::now_utc(),
        expires_at,
        source,
        delete: ShareDelete::FreeToken("token".into()),
        state: ShareState::Ready,
    }
}

fn request(source: ShareSource) -> ShareStart {
    ShareStart {
        media: Media {
            kind: ShareKind::Image,
            mime: "image/png".into(),
            path: PathBuf::from("/nowhere/shot.png"),
            width: 4,
            height: 3,
            duration_ms: None,
            poster: None,
        },
        title: None,
        days: 7,
        source,
    }
}

fn history_entry(share_url: Option<String>) -> HistoryEntry {
    HistoryEntry {
        id: Uuid::new_v4(),
        created_at: OffsetDateTime::now_utc(),
        kind: CaptureKind::Region,
        name: "shot".into(),
        file: PathBuf::from("/elsewhere/shot.png"),
        export_path: None,
        pixel_size: (4, 3),
        has_annotations: false,
        share_url,
    }
}

struct Rig {
    _dir: tempfile::TempDir,
    paths: Paths,
    settings: Entity<SettingsModel>,
    history: Entity<History>,
    shares: Entity<Shares>,
    clip: Rc<Clip>,
    events: Rc<RefCell<Vec<ShareJobEvent>>>,
    deletes: Rc<RefCell<Vec<ShareDeleted>>>,
}

fn rig(cx: &mut TestAppContext, fake: &Arc<Fake>, prepare: impl FnOnce(&Paths)) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    prepare(&paths);
    let clip = Rc::new(Clip::default());
    let events = Rc::new(RefCell::new(Vec::new()));
    let deletes = Rc::new(RefCell::new(Vec::new()));
    let (settings, history, shares) = cx.update(|cx| {
        let config = ConfigFile::load(&paths.config_file()).unwrap();
        let settings = cx.new(|_| SettingsModel::new(config));
        let history = cx.new(|_| History::open(&paths, 10).unwrap());
        let targets = Arc::new(FakeTargets(fake.clone()));
        let shares = cx.new(|cx| {
            Shares::new(
                &paths,
                settings.clone(),
                history.clone(),
                clip.clone(),
                targets,
                cx,
            )
        });
        let sink = events.clone();
        cx.subscribe(&shares, move |_, e: &ShareJobEvent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        let sink = deletes.clone();
        cx.subscribe(&shares, move |_, e: &ShareDeleted, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
        (settings, history, shares)
    });
    Rig {
        _dir: dir,
        paths,
        settings,
        history,
        shares,
        clip,
        events,
        deletes,
    }
}

fn consent(rig: &Rig, cx: &mut TestAppContext) {
    rig.shares
        .update(cx, |s, cx| s.consent(ShareMode::Free, cx));
}

fn kinds(rig: &Rig) -> Vec<ShareJobKind> {
    rig.events.borrow().iter().map(|e| e.kind.clone()).collect()
}

fn wait_until(cx: &mut TestAppContext, what: &str, mut done: impl FnMut() -> bool) {
    cx.executor().allow_parking();
    for _ in 0..200 {
        cx.run_until_parked();
        if done() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

fn finished(rig: &Rig) -> bool {
    kinds(rig)
        .iter()
        .any(|k| matches!(k, ShareJobKind::Done(_) | ShareJobKind::Failed(_)))
}

fn on_disk(paths: &Paths) -> Vec<String> {
    let file = es_store::shares::open(paths).unwrap();
    file.value.entries.into_iter().map(|r| r.id).collect()
}

#[gpui::test]
fn nothing_is_sent_before_the_user_chooses(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    let source = ShareSource::Capture(Uuid::new_v4());
    rig.shares.update(cx, |s, cx| s.start(request(source), cx));
    cx.run_until_parked();
    assert_eq!(
        kinds(&rig),
        vec![ShareJobKind::Failed(ShareFailure::NoConsent)]
    );
    assert_eq!(
        fake.opened.load(Ordering::SeqCst),
        0,
        "no target may be reached without consent"
    );
}

#[gpui::test]
fn the_link_is_copied_when_reserved_before_the_upload_finishes(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    consent(&rig, cx);
    let source = ShareSource::Capture(Uuid::new_v4());
    let id = rig.shares.update(cx, |s, cx| s.start(request(source), cx));
    wait_until(cx, "the share to finish", || finished(&rig));
    let kinds = kinds(&rig);
    assert!(rig.events.borrow().iter().all(|e| e.id == id));
    assert_eq!(
        kinds[..2],
        [
            ShareJobKind::Reserved {
                page_url: PAGE.into()
            },
            ShareJobKind::Progress { sent: 5, total: 10 },
        ]
    );
    assert!(matches!(kinds[2], ShareJobKind::Done(_)), "{kinds:?}");
    assert_eq!(*rig.clip.0.borrow(), vec![ClipItem::Text(PAGE.into())]);
}

#[gpui::test]
fn a_finished_share_is_saved_before_done_and_shown_in_history(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    consent(&rig, cx);
    let entry = history_entry(None);
    let capture = entry.id;
    rig.history.update(cx, |h, cx| h.add(entry, cx).unwrap());
    let saved_at_done = Rc::new(RefCell::new(None));
    let paths = rig.paths.clone();
    let seen = saved_at_done.clone();
    cx.update(|cx| {
        cx.subscribe(&rig.shares, move |_, e: &ShareJobEvent, _| {
            if matches!(e.kind, ShareJobKind::Done(_)) {
                *seen.borrow_mut() = Some(on_disk(&paths));
            }
        })
        .detach();
    });
    let source = ShareSource::Capture(capture);
    rig.shares
        .update(cx, |s, cx| s.start(request(source.clone()), cx));
    wait_until(cx, "the share to finish", || finished(&rig));
    assert_eq!(
        *saved_at_done.borrow(),
        Some(vec!["abc123".to_owned()]),
        "a link the user has must survive a crash right after Done"
    );
    let url = rig
        .history
        .read_with(cx, |h, _| h.get(capture).unwrap().share_url.clone());
    assert_eq!(url.as_deref(), Some("https://share.example/s/abc123"));
    let found = rig
        .shares
        .read_with(cx, |s, _| s.for_source(&source).map(|r| r.id.clone()));
    assert_eq!(found.as_deref(), Some("abc123"));
}

#[gpui::test]
fn a_refusal_keeps_its_wait_and_is_told_in_the_apps_words(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|_| {
        Err(ShareError::Service {
            code: Code::RateLimited,
            retry_after_s: Some(290),
        })
    }));
    let rig = rig(cx, &fake, |_| {});
    consent(&rig, cx);
    rig.shares.update(cx, |s, cx| {
        s.start(request(ShareSource::Capture(Uuid::nil())), cx)
    });
    wait_until(cx, "the share to fail", || finished(&rig));
    let failure = ShareFailure::Refused {
        code: Code::RateLimited,
        retry_after_s: Some(290),
    };
    assert_eq!(kinds(&rig).last(), Some(&ShareJobKind::Failed(failure)));
    assert_eq!(
        failure.message(&Strings),
        "Too many shares in a short time. Try again in 5 minutes."
    );
    assert!(on_disk(&rig.paths).is_empty());
}

#[gpui::test]
fn cancel_answers_at_once_and_a_late_finish_is_deleted(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let (open, gate) = mpsc::channel();
    *fake.gate.lock().unwrap() = Some(gate);
    let rig = rig(cx, &fake, |_| {});
    consent(&rig, cx);
    let id = rig.shares.update(cx, |s, cx| {
        s.start(request(ShareSource::Capture(Uuid::nil())), cx)
    });
    wait_until(cx, "the progress", || kinds(&rig).len() == 2);

    assert!(rig.shares.update(cx, |s, cx| s.cancel(id, cx)));
    assert_eq!(kinds(&rig).last(), Some(&ShareJobKind::Cancelled));
    assert!(
        !rig.shares.update(cx, |s, cx| s.cancel(id, cx)),
        "cancelled once"
    );

    open.send(()).unwrap();
    wait_until(cx, "the finished share to be deleted", || {
        *fake.deleted.lock().unwrap() == ["abc123"]
    });
    assert_eq!(
        kinds(&rig).len(),
        3,
        "nothing after Cancelled: {:?}",
        kinds(&rig)
    );
    assert!(
        on_disk(&rig.paths).is_empty(),
        "a cancelled share is not kept"
    );
}

#[gpui::test]
fn delete_forgets_the_share_when_done_or_already_gone(cx: &mut TestAppContext) {
    let fake = Fake::deleting(Box::new(|r| match r.id.as_str() {
        "gone" => Err(ShareError::Service {
            code: Code::NotFound,
            retry_after_s: None,
        }),
        "offline" => Err(ShareError::Network("timeout".into())),
        _ => Ok(()),
    }));
    let entry = history_entry(Some("https://share.example/s/live".into()));
    let capture = entry.id;
    let rig = rig(cx, &fake, |paths| {
        let mut file = es_store::shares::open(paths).unwrap();
        file.value
            .upsert(record("offline", ShareSource::Capture(Uuid::nil()), None));
        file.value
            .upsert(record("gone", ShareSource::Capture(Uuid::nil()), None));
        file.value
            .upsert(record("live", ShareSource::Capture(capture), None));
        file.save().unwrap();
    });
    rig.history.update(cx, |h, cx| h.add(entry, cx).unwrap());
    for id in ["live", "gone", "offline"] {
        let source = ShareSource::Capture(if id == "live" { capture } else { Uuid::nil() });
        rig.shares
            .update(cx, |s, cx| s.delete(record(id, source, None), cx));
    }
    wait_until(cx, "three deletes", || rig.deletes.borrow().len() == 3);
    let results: Vec<_> = rig
        .deletes
        .borrow()
        .iter()
        .map(|d| (d.record.id.clone(), d.result))
        .collect();
    assert!(results.contains(&("live".into(), Ok(()))));
    assert!(results.contains(&("gone".into(), Ok(()))));
    assert!(results.contains(&("offline".into(), Err(ShareFailure::Network))));
    assert_eq!(
        on_disk(&rig.paths),
        ["offline"],
        "a failed delete keeps the record"
    );
    let url = rig
        .history
        .read_with(cx, |h, _| h.get(capture).unwrap().share_url.clone());
    assert_eq!(url, None, "the gallery must not offer a deleted link");
}

#[gpui::test]
fn expired_free_links_are_dropped_on_load(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let past = OffsetDateTime::now_utc() - time::Duration::hours(1);
    let future = OffsetDateTime::now_utc() + time::Duration::days(1);
    let rig = rig(cx, &fake, |paths| {
        let mut file = es_store::shares::open(paths).unwrap();
        file.value.upsert(record(
            "dead",
            ShareSource::Capture(Uuid::nil()),
            Some(past),
        ));
        file.value.upsert(record(
            "alive",
            ShareSource::Capture(Uuid::nil()),
            Some(future),
        ));
        let mut own = record("bucket", ShareSource::Capture(Uuid::nil()), None);
        own.target = ShareTarget::Bucket { label: "b".into() };
        own.delete = ShareDelete::Bucket;
        file.value.upsert(own);
        file.save().unwrap();
    });
    let mut kept: Vec<String> = rig
        .shares
        .read_with(cx, |s, _| s.all().iter().map(|r| r.id.clone()).collect());
    kept.sort();
    assert_eq!(kept, ["alive", "bucket"]);
    let mut stored = on_disk(&rig.paths);
    stored.sort();
    assert_eq!(stored, ["alive", "bucket"]);
}

#[gpui::test]
fn the_popover_choices_are_written_to_the_share_section(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    rig.shares.update(cx, |s, cx| {
        s.consent(ShareMode::Bucket, cx);
        assert!(s.set_days(3, cx));
        assert!(!s.set_days(5, cx), "the service offers 1, 3 and 7 days");
        assert_eq!(s.days(cx), 3);
        s.set_target(ShareMode::Free, cx);
    });
    let share = cx.update(|cx| rig.settings.read(cx).config().share.clone());
    assert_eq!(
        (share.target, share.consented, share.days),
        (ShareMode::Free, true, 3)
    );
    rig.settings.update(cx, |s, cx| s.flush(cx));
    let text = std::fs::read_to_string(rig.paths.config_file()).unwrap();
    assert!(
        text.contains("[share]") && text.contains("consented = true"),
        "{text}"
    );
}

#[gpui::test]
fn a_target_that_dies_still_ends_its_job(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|_| panic!("the target died")));
    let rig = rig(cx, &fake, |_| {});
    consent(&rig, cx);
    rig.shares.update(cx, |s, cx| {
        s.start(request(ShareSource::Capture(Uuid::nil())), cx)
    });
    wait_until(cx, "the job to end", || finished(&rig));
    assert_eq!(
        kinds(&rig).last(),
        Some(&ShareJobKind::Failed(ShareFailure::Unexpected))
    );
}
