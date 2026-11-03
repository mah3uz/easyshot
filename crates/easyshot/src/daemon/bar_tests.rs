#![allow(clippy::unwrap_used)]
use super::bar::start_recording;
use super::recording::{Deps, RecordingUi, Recordings, Shown};
use es_app::deck::{Deck, DeckPolicy};
use es_app::errors::Error;
use es_app::settings_model::SettingsModel;
use es_app::toast::{MemorySink, install_toasts};
use es_desktop::{Notification, NotificationId, Notifier, RecordingTrayState};
use es_model::platform::OutputId;
use es_overlays::record::{ControlBar, Recorded, RecordingRequest, RecordingTarget};
use es_record::Clock;
use es_record::project::CommandRunner;
use es_store::{ConfigFile, Paths};
use gpui::{App, AppContext as _, Entity, Task, TestAppContext};
use std::process::Output;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

struct Silent;

impl Notifier for Silent {
    fn notify(&self, _: Notification) -> es_desktop::Result<NotificationId> {
        Ok(NotificationId(1))
    }
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        async_channel::unbounded().1
    }
    fn supports_actions(&self) -> bool {
        false
    }
}

struct NoUi;

impl RecordingUi for NoUi {
    fn show(&self, _: &Recorded, _: bool, _: &Entity<ControlBar>, _: &mut App) -> Box<dyn Shown> {
        unreachable!("no start gets this far")
    }
    fn tray(&self, _: Option<RecordingTrayState>, _: &mut App) {}
    fn open_trim(&self, _: std::path::PathBuf, _: String, _: &mut App) {}
}

struct NoClock;

impl Clock for NoClock {
    fn now(&self) -> Duration {
        Duration::ZERO
    }
    fn sleep(&self, _: Duration) {}
    fn monotonic_us(&self) -> u64 {
        0
    }
}

struct NoTools;

impl CommandRunner for NoTools {
    fn run(&self, _: &str, _: &[String]) -> std::io::Result<Output> {
        Err(std::io::Error::other("no tools"))
    }
}

fn request(delay_secs: u32) -> RecordingRequest {
    RecordingRequest {
        target: RecordingTarget::Display(OutputId("A".into())),
        mic: None,
        system_audio: false,
        delay_secs,
    }
}

fn recordings(cx: &mut TestAppContext, recorder_missing: bool) -> (Recordings, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let recordings = cx.update(|cx| {
        let file = ConfigFile::load(&paths.config_file()).unwrap();
        Recordings::new(Deps {
            settings: cx.new(|_| SettingsModel::new(file)),
            paths,
            deck: cx.new(|_| Deck::new(DeckPolicy::default())),
            recorder: Rc::new(|_| Err(Error::Unsupported)),
            recorder_missing,
            outputs: Rc::new(Vec::new),
            ui: Rc::new(NoUi),
            clock: Arc::new(NoClock),
            runner: Arc::new(NoTools),
            countdown: Rc::new(|_, cx| cx.spawn(async move |_| std::future::pending().await)),
            select_area: Rc::new(|_| Task::ready(Err(Error::Cancelled))),
        })
    });
    (recordings, dir)
}

fn titles(sink: &MemorySink) -> Vec<String> {
    sink.shown().into_iter().map(|t| t.title).collect()
}

#[gpui::test]
fn a_start_refused_for_a_missing_recorder_shows_only_that_toast(cx: &mut TestAppContext) {
    let (recordings, _dir) = recordings(cx, true);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    cx.update(|cx| start_recording(&recordings, request(0), cx));
    cx.run_until_parked();
    assert_eq!(titles(&sink), ["No screen recorder is installed"]);
}

#[gpui::test]
fn a_start_refused_for_a_running_recording_shows_only_that_toast(cx: &mut TestAppContext) {
    let (recordings, _dir) = recordings(cx, false);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    cx.update(|cx| start_recording(&recordings, request(5), cx));
    cx.run_until_parked();
    assert!(
        titles(&sink).is_empty(),
        "the first start waits in its countdown"
    );
    cx.update(|cx| start_recording(&recordings, request(0), cx));
    cx.run_until_parked();
    assert_eq!(titles(&sink), ["A recording is already running"]);
}

#[gpui::test]
fn a_start_that_fails_after_admission_still_shows_the_generic_toast(cx: &mut TestAppContext) {
    let (recordings, _dir) = recordings(cx, false);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    cx.update(|cx| start_recording(&recordings, request(0), cx));
    cx.run_until_parked();
    assert_eq!(titles(&sink), ["Something went wrong"]);
}
