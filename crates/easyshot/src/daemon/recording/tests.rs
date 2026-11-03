#![allow(clippy::unwrap_used)]
use super::fake::FakeRecorder;
use super::*;
use es_app::deck::DeckPolicy;
use es_record::project::CommandRunner;
use es_record::{Clock, recovery};
use es_store::ConfigFile;
use gpui::TestAppContext;
use std::cell::Cell;
use std::os::unix::process::ExitStatusExt as _;
use std::process::{ExitStatus, Output};
use std::sync::Mutex;

struct ManualClock(Mutex<Duration>);

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }
    fn sleep(&self, _: Duration) {}
    fn monotonic_us(&self) -> u64 {
        0
    }
}

struct NoTools;

impl CommandRunner for NoTools {
    fn run(&self, _: &str, _: &[String]) -> std::io::Result<Output> {
        Ok(Output {
            status: ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

#[derive(Default)]
struct Log {
    tray: Vec<Option<RecordingTrayState>>,
    trimmed: Vec<std::path::PathBuf>,
    shown: usize,
    closed: usize,
}

struct FakeUi(Rc<RefCell<Log>>);

struct FakeShown(Rc<RefCell<Log>>);

impl Shown for FakeShown {
    fn resized(&self, _: &Entity<ControlBar>, _: &mut App) {}
    fn close(&self, _: &mut App) {
        self.0.borrow_mut().closed += 1;
    }
}

impl RecordingUi for FakeUi {
    fn show(&self, _: &Recorded, _: bool, _: &Entity<ControlBar>, _: &mut App) -> Box<dyn Shown> {
        self.0.borrow_mut().shown += 1;
        Box::new(FakeShown(self.0.clone()))
    }
    fn tray(&self, state: Option<RecordingTrayState>, _: &mut App) {
        self.0.borrow_mut().tray.push(state);
    }
    fn open_trim(&self, video: std::path::PathBuf, _: String, _: &mut App) {
        self.0.borrow_mut().trimmed.push(video);
    }
}

struct Rig {
    recordings: Recordings,
    starts: Rc<Cell<usize>>,
    picks: Rc<Cell<usize>>,
    countdown_done: async_channel::Sender<()>,
    deck: Entity<Deck>,
    clock: Arc<ManualClock>,
    log: Rc<RefCell<Log>>,
    paths: Paths,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn advance(&self, secs: u64) {
        *self.clock.0.lock().unwrap() += Duration::from_secs(secs);
    }

    fn folders(&self) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(self.paths.recordings_dir())
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    }
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let clock = Arc::new(ManualClock(Mutex::new(Duration::from_secs(100))));
    let log = Rc::new(RefCell::new(Log::default()));
    let (starts, picks) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (countdown_done, countdown_wait) = async_channel::unbounded::<()>();
    let (counted, picked) = (starts.clone(), picks.clone());
    let (recordings, deck) = cx.update(|cx| {
        let file = ConfigFile::load(&dir.path().join("config.toml")).unwrap();
        let settings = cx.new(|_| SettingsModel::new(file));
        let deck = cx.new(|_| Deck::new(DeckPolicy::default()));
        let recordings = Recordings::new(Deps {
            settings,
            paths: paths.clone(),
            deck: deck.clone(),
            recorder: Rc::new(move |_| {
                counted.set(counted.get() + 1);
                Ok(Arc::new(FakeRecorder::default()))
            }),
            outputs: Rc::new(Vec::new),
            ui: Rc::new(FakeUi(log.clone())),
            clock: clock.clone(),
            runner: Arc::new(NoTools),
            countdown: Rc::new(move |_, cx| {
                let wait = countdown_wait.clone();
                cx.spawn(async move |_| wait.recv().await.map_err(|_| Error::Cancelled))
            }),
            select_area: Rc::new(move |_| {
                picked.set(picked.get() + 1);
                Task::ready(Err(Error::Cancelled))
            }),
        });
        (recordings, deck)
    });
    Rig {
        recordings,
        starts,
        picks,
        countdown_done,
        deck,
        clock,
        log,
        paths,
        _dir: dir,
    }
}

fn display() -> Start {
    Start {
        request: RecordingRequest {
            target: RecordingTarget::Display(OutputId("DP-1".into())),
            mic: None,
            system_audio: false,
            delay_secs: 0,
        },
        area: None,
    }
}

fn cards(rig: &Rig, cx: &mut TestAppContext) -> Vec<DeckItem> {
    cx.update(|cx| rig.deck.read(cx).items().cloned().collect())
}

#[gpui::test]
async fn pause_is_not_recorded_and_stop_leaves_a_video_card(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    rig.advance(5);
    cx.update(|cx| rig.recordings.toggle_pause(cx)).unwrap();
    cx.run_until_parked();
    rig.advance(10);
    cx.update(|cx| rig.recordings.toggle_pause(cx)).unwrap();
    cx.run_until_parked();
    rig.advance(2);
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();

    assert!(!rig.recordings.is_recording());
    let cards = cards(&rig, cx);
    assert_eq!(cards.len(), 1);
    assert!(cards[0].video && !cards[0].staged);
    let folder = rig.folders().remove(0);
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("capture.json")).unwrap()).unwrap();
    assert_eq!(
        capture["duration_secs"], 7.0,
        "5 s + 2 s, the 10 s pause left out"
    );
    let log = rig.log.borrow();
    assert_eq!((log.shown, log.closed), (1, 1));
    assert!(log.tray.iter().any(|t| t.is_some_and(|s| s.paused)));
    assert_eq!(
        log.tray.last(),
        Some(&None),
        "the tray leaves the recording state"
    );
}

#[gpui::test]
async fn discard_keeps_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(rig.folders().len(), 1);
    cx.update(|cx| rig.recordings.discard(cx)).unwrap();
    cx.run_until_parked();
    assert!(rig.folders().is_empty());
    assert!(cards(&rig, cx).is_empty());
    assert!(!rig.recordings.is_recording());
}

#[gpui::test]
async fn start_over_replaces_the_recording_and_keeps_recording(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    let first = rig.folders();
    cx.update(|cx| rig.recordings.restart(cx)).unwrap();
    cx.run_until_parked();
    let second = rig.folders();
    assert!(rig.recordings.is_recording());
    assert_eq!(second.len(), 1);
    assert_ne!(first, second, "a fresh folder, the old one deleted");
    assert_eq!(rig.log.borrow().shown, 2);
}

#[gpui::test]
async fn controls_refuse_when_none_runs_and_the_tray_menu_leads_with_stop(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert!(cx.update(|cx| rig.recordings.stop(cx)).is_err());
    assert!(cx.update(|cx| rig.recordings.toggle_pause(cx)).is_err());
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    assert!(cx.update(|cx| rig.recordings.start(display(), cx)).is_err());
    cx.update(|cx| rig.recordings.toggle_pause(cx)).unwrap();
    cx.run_until_parked();
    let menu = es_shell::tray::menu(&es_shell::tray::TrayInput {
        recording: rig.log.borrow().tray.last().copied().flatten(),
        ..Default::default()
    });
    let routes: Vec<_> = menu.items.iter().filter_map(|i| i.route.clone()).collect();
    assert_eq!(
        routes[..4],
        [
            Route::RecordStop,
            Route::RecordPause,
            Route::RecordRestart,
            Route::RecordDiscard
        ]
    );
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(cards(&rig, cx).len(), 1);
}

#[gpui::test]
async fn a_recovered_recording_becomes_a_card(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let dir = rig.paths.recordings_dir().join("EasyShot_x.easyshotrec");
    let found = [recovery::Recovered {
        dir: dir.clone(),
        capture: Default::default(),
    }];
    cx.update(|cx| rig.recordings.show_recovered(&found, cx));
    let cards = cards(&rig, cx);
    assert_eq!(cards.len(), 1);
    assert!(cards[0].video);
    assert_eq!(cards[0].file, dir.join("screen.mkv"));
    assert_eq!(cards[0].thumb, dir.join("poster.jpg"));
}

async fn stopped_recording_trimmed(open_editor_after: bool, cx: &mut TestAppContext) -> usize {
    let rig = rig(cx);
    let settings = cx.update(|_| rig.recordings.0.deps.settings.clone());
    cx.update(|cx| {
        settings.update(cx, |s, cx| {
            s.update(|c| c.recording.open_editor_after = open_editor_after, cx)
        })
    });
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    rig.advance(3);
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();
    rig.log.borrow().trimmed.len()
}

#[gpui::test]
async fn stop_opens_the_trim_viewer_only_when_asked(cx: &mut TestAppContext) {
    assert_eq!(stopped_recording_trimmed(true, cx).await, 1);
    assert_eq!(stopped_recording_trimmed(false, cx).await, 0);
}

fn area() -> RecordingRequest {
    RecordingRequest {
        target: RecordingTarget::Area,
        ..display().request
    }
}

#[gpui::test]
async fn a_second_start_is_refused_from_every_path_while_one_records(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(rig.starts.get(), 1);

    let routed = cx.update(|cx| rig.recordings.admit(cx));
    let launched = cx.update(|cx| rig.recordings.launch(display(), cx));
    let bar_display = cx.update(|cx| rig.recordings.with_request(display().request, cx));
    let bar_area = cx.update(|cx| rig.recordings.with_request(area(), cx));
    let again = cx.update(|cx| rig.recordings.start(display(), cx));
    assert!(
        routed.is_err(),
        "tray, shortcut, CLI and link are refused at the gate"
    );
    assert!(launched.await.is_err());
    assert!(bar_display.await.is_err());
    assert!(bar_area.await.is_err());
    assert!(again.is_err());
    assert_eq!(
        rig.starts.get(),
        1,
        "the recorder never started a second time"
    );
    assert_eq!(
        rig.picks.get(),
        0,
        "no area picker opens over a running recording"
    );
    assert!(rig.recordings.is_recording());
}

#[gpui::test]
async fn a_start_counting_down_already_holds_the_recorder(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let delayed = Start {
        request: RecordingRequest {
            delay_secs: 3,
            ..display().request
        },
        area: None,
    };
    let first = cx.update(|cx| rig.recordings.launch(delayed, cx));
    cx.run_until_parked();
    assert!(!rig.recordings.is_recording());
    assert!(cx.update(|cx| rig.recordings.admit(cx)).is_err());
    let second = cx.update(|cx| rig.recordings.launch(display(), cx));
    assert!(second.await.is_err());
    rig.countdown_done.try_send(()).unwrap();
    first.await.unwrap();
    assert_eq!(rig.starts.get(), 1);
    assert!(rig.recordings.is_recording());
}

#[gpui::test]
async fn a_cancelled_area_pick_frees_the_recorder(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let picked = cx.update(|cx| rig.recordings.with_request(area(), cx));
    assert!(matches!(picked.await, Err(Error::Cancelled)));
    assert!(!rig.recordings.is_busy());
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    assert_eq!(rig.starts.get(), 1);
}

#[test]
fn exports_get_dated_names_in_the_videos_folder_that_never_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let template = es_model::Config::default().general.file_name_template;
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let first = finish::video_target(&paths, &template, "mp4", now).unwrap();
    std::fs::write(&first, b"video").unwrap();
    let second = finish::video_target(&paths, &template, "mp4", now).unwrap();
    assert!(first.starts_with(paths.videos_dir()) && second.starts_with(paths.videos_dir()));
    assert!(
        !first.starts_with(paths.recordings_dir()),
        "project data stays apart"
    );
    assert_ne!(
        first, second,
        "a second export in the same second keeps the first"
    );
    assert_eq!(first.extension().unwrap(), "mp4");
    let name = first.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.contains("2027"), "the name carries the date: {name}");
}
