#![allow(clippy::unwrap_used)]
use super::*;
use crate::daemon::agent::gate::tests::{GateRig, ME};
use crate::daemon::agent::handler_full;
use crate::daemon::recording::fake::FakeRecorder;
use crate::daemon::recording::{Deps, RecordingUi, Shown, input::Inputs};
use es_app::deck::{Deck, DeckPolicy};
use es_app::events::Origin;
use es_app::jobs::{JobId, Jobs};
use es_backend::Caps;
use es_desktop::RecordingTrayState;
use es_model::AgentCfg;
use es_model::platform::{Transform, WindowId};
use es_overlays::record::{ControlBar, Recorded};
use es_protocol::AgentRoute;
use es_protocol::agent::{Audio, Geometry, GrantSpan, WindowSelector};
use es_record::Clock;
use es_record::project::CommandRunner;
use es_store::Paths;
use gpui::{AppContext as _, TestAppContext};
use serde_json::Value;
use std::cell::Cell;
use std::process::Output;
use std::sync::{Arc, Mutex};

struct ManualClock(Mutex<Duration>);

impl Clock for ManualClock {
    fn now(&self) -> Duration {
        *self.0.lock().unwrap()
    }
    fn sleep(&self, _: Duration) {}
    fn monotonic_us(&self) -> u64 {
        self.now().as_micros() as u64
    }
}

struct NoTools;

impl CommandRunner for NoTools {
    fn run(&self, _: &str, _: &[String]) -> std::io::Result<Output> {
        Err(std::io::Error::other("no tools"))
    }
}

struct FakeUi;

struct FakeShown;

impl Shown for FakeShown {
    fn resized(&self, _: &Entity<ControlBar>, _: bool, _: &mut App) {}
    fn close(&self, _: &mut App) {}
}

impl RecordingUi for FakeUi {
    fn show(&self, _: &Recorded, _: bool, _: &Entity<ControlBar>, _: &mut App) -> Box<dyn Shown> {
        Box::new(FakeShown)
    }
    fn tray(&self, _: Option<RecordingTrayState>, _: &mut App) {}
    fn open_trim(&self, _: std::path::PathBuf, _: String, _: &mut App) {}
}

fn output(name: &str, logical: Rect, focused: bool) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical,
        physical: (800, 600),
        scale: 2.0,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: focused.then_some(true),
    }
}

fn outputs() -> Vec<OutputInfo> {
    vec![
        output("DP-1", Rect::new(0., 0., 400., 300.), false),
        output("DP-2", Rect::new(400., 0., 800., 300.), true),
    ]
}

fn window(id: &str, app_id: &str, on: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: app_id.into(),
        title: id.into(),
        pid: None,
        logical: Some(Rect::new(100., 50., 300., 150.)),
        output: Some(OutputId(on.into())),
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

fn desk() -> Vec<WindowInfo> {
    vec![window("1", "foot", "DP-1"), window("2", "mpv", "DP-2")]
}

fn sources() -> Sources {
    Sources {
        caps: Caps::all(),
        outputs: Arc::new(outputs),
        windows: Arc::new(desk),
    }
}

struct Rig {
    gate: GateRig,
    recordings: Recordings,
    coordinator: es_app::capture_coordinator::CaptureCoordinator,
    clock: Arc<ManualClock>,
    handler: es_app::dispatcher::AgentHandler,
    finished: async_channel::Receiver<es_app::jobs::JobFinished>,
    next: Cell<u64>,
    paths: Paths,
    _dir: tempfile::TempDir,
}

fn rig_for(agent: AgentCfg, cx: &mut TestAppContext) -> Rig {
    let unreachable: crate::daemon::recording::FocusWindow =
        Rc::new(|_, _| Task::ready(Err(es_app::errors::Error::WindowUnreachable)));
    rig_with(agent, sources(), unreachable, cx)
}

fn rig_with(
    agent: AgentCfg,
    sources: Sources,
    focus_window: crate::daemon::recording::FocusWindow,
    cx: &mut TestAppContext,
) -> Rig {
    let gate = GateRig::new(agent, cx);
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let clock = Arc::new(ManualClock(Mutex::new(Duration::from_secs(100))));
    let (jobs, finished) = Jobs::new();
    let coordinator = es_app::capture_coordinator::CaptureCoordinator::new(jobs.clone());
    let (recordings, handler) = cx.update(|cx| {
        let recordings = Recordings::new(Deps {
            settings: gate.settings.clone(),
            paths: paths.clone(),
            deck: cx.new(|_| Deck::new(DeckPolicy::default())),
            recorder: Rc::new(|_| Ok(Arc::new(FakeRecorder::default()))),
            recorder_missing: false,
            outputs: Rc::new(outputs),
            ui: Rc::new(FakeUi),
            clock: clock.clone(),
            runner: Arc::new(NoTools),
            countdown: Rc::new(|_, cx| cx.spawn(async move |_| Ok(()))),
            select_area: Rc::new(|_| Task::ready(Err(es_app::errors::Error::Cancelled))),
            focus_window,
            input: Inputs {
                open: Rc::new(|| Err(es_model::recording::InputAccess::Denied)),
                sources: es_record::input::Sources {
                    cursor: None,
                    keymap: None,
                    session_open: Arc::default(),
                },
            },
        });
        let record = install(
            cx,
            &recordings,
            &coordinator,
            sources.clone(),
            &gate.access,
            &gate.settings,
        );
        let handler = handler_full(
            sources,
            jobs.clone(),
            gate.gate.clone(),
            None,
            None,
            Some(record),
        );
        (recordings, handler)
    });
    Rig {
        gate,
        recordings,
        coordinator,
        clock,
        handler,
        finished,
        next: Cell::new(1),
        paths,
        _dir: dir,
    }
}

fn rig(cx: &mut TestAppContext) -> Rig {
    rig_for(
        AgentCfg {
            enabled: true,
            consent: es_model::Consent::AlwaysAllow,
            ..AgentCfg::default()
        },
        cx,
    )
}

impl Rig {
    fn advance(&self, secs: u64) {
        *self.clock.0.lock().unwrap() += Duration::from_secs(secs);
    }

    fn submit(&self, route: RecordRoute, cx: &mut TestAppContext) -> JobId {
        let job = JobId(self.next.replace(self.next.get() + 1));
        self.gate.gate.callers().borrow_mut().insert(
            job,
            es_desktop::dbus::Caller {
                uid: Some(ME),
                pid: None,
            },
        );
        let route = AgentRoute::Record(route);
        cx.update(|cx| (self.handler)(job, route, Origin::DBus, cx));
        job
    }

    fn answer(&self) -> Value {
        let done = self.finished.try_recv().expect("the job finishes");
        serde_json::from_str(done.detail.as_deref().unwrap()).unwrap()
    }

    fn ask(&self, route: RecordRoute, cx: &mut TestAppContext) -> Value {
        self.submit(route, cx);
        cx.run_until_parked();
        self.answer()
    }

    fn start(&self, target: AgentTarget, max_seconds: u32, cx: &mut TestAppContext) -> Value {
        let spec = RecordSpec {
            max_seconds,
            ..RecordSpec::new(target)
        };
        self.ask(RecordRoute::Start(spec), cx)
    }

    fn recording(&self) -> bool {
        self.recordings.is_recording()
    }
}

fn code(reply: &Value) -> &str {
    reply["error"]["code"].as_str().unwrap_or("none")
}

#[gpui::test]
async fn a_started_recording_stops_with_the_video_path_and_its_length(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let started = rig.start(AgentTarget::Fullscreen, 120, cx);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["kind"], "recording");
    assert_eq!(started["state"], "recording");
    assert_eq!(started["target"], "fullscreen (DP-2)");
    let id = started["id"].as_str().unwrap().to_owned();
    assert!(rig.recording());
    rig.advance(7);

    let status = rig.ask(RecordRoute::Status, cx);
    assert_eq!(
        (status["state"].as_str(), status["id"].as_str()),
        (Some("recording"), Some(id.as_str()))
    );
    assert_eq!(status["elapsed_ms"], 7000, "{status}");

    let stopped = rig.ask(RecordRoute::Stop(None), cx);
    assert_eq!(stopped["ok"], true, "{stopped}");
    assert_eq!(stopped["id"], id.as_str());
    assert_eq!(stopped["duration_ms"], 7000);
    let path = std::path::PathBuf::from(stopped["path"].as_str().unwrap());
    assert!(path.starts_with(rig.paths.recordings_dir()), "{path:?}");
    assert_eq!(stopped["bytes"], std::fs::metadata(&path).unwrap().len());
    assert!(!rig.recording());
    let idle = rig.ask(RecordRoute::Status, cx);
    assert_eq!(idle["state"], "idle");
}

#[gpui::test]
async fn a_second_start_while_one_runs_is_invalid_arguments(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.start(AgentTarget::Fullscreen, 120, cx);
    let again = rig.start(AgentTarget::Fullscreen, 120, cx);
    assert_eq!(code(&again), "invalid_arguments", "{again}");
    assert_eq!(
        again["error"]["hint"],
        "A recording is already running. Stop it, then try again."
    );
    assert!(
        again["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already running")
    );
    let status = rig.ask(RecordRoute::Status, cx);
    assert_eq!(
        status["id"], first["id"],
        "the first recording is untouched"
    );
    assert_eq!(
        std::fs::read_dir(rig.paths.recordings_dir())
            .unwrap()
            .count(),
        1
    );
}

#[gpui::test]
async fn ending_the_grant_stops_the_recording_and_keeps_its_video(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.gate
        .access
        .update(cx, |access, cx| access.allow(GrantSpan::Session, cx));
    let id = rig.start(AgentTarget::Fullscreen, 120, cx)["id"].clone();
    rig.advance(3);
    rig.gate.access.update(cx, |access, cx| access.revoke(cx));
    cx.run_until_parked();
    assert!(
        !rig.recording(),
        "the revoked grant left a recording running"
    );

    let kept = rig.ask(RecordRoute::Stop(Some(id.as_str().unwrap().into())), cx);
    assert_eq!(kept["ok"], true, "{kept}");
    assert_eq!(kept["duration_ms"], 3000);
    assert!(std::path::Path::new(kept["path"].as_str().unwrap()).is_file());
}

#[gpui::test]
async fn turning_agent_access_off_stops_the_recording(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.start(AgentTarget::Fullscreen, 120, cx);
    rig.gate
        .settings
        .update(cx, |s, cx| s.update(|c| c.agent.enabled = false, cx));
    cx.run_until_parked();
    assert!(!rig.recording());
}

#[gpui::test]
async fn a_recording_stops_by_itself_after_its_longest_time(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let id = rig.start(AgentTarget::Fullscreen, 5, cx)["id"].clone();
    rig.advance(4);
    cx.executor().advance_clock(Duration::from_secs(4));
    cx.run_until_parked();
    assert!(rig.recording(), "stopped before its time");
    rig.advance(1);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert!(!rig.recording(), "ran past its longest time");

    let kept = rig.ask(RecordRoute::Stop(None), cx);
    assert_eq!(
        (kept["id"].clone(), kept["duration_ms"].clone()),
        (id, 5000.into()),
        "{kept}"
    );
}

#[gpui::test]
async fn a_recording_that_nobody_started_for_an_agent_is_not_the_agents_to_stop(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let own = Start::new(
        es_overlays::record::RecordingRequest {
            target: RecordingTarget::Display(OutputId("DP-1".into())),
            mic: None,
            system_audio: false,
            delay_secs: 0,
        },
        None,
    );
    cx.update(|cx| rig.recordings.start(own, cx)).unwrap();
    for route in [RecordRoute::Stop(None), RecordRoute::Discard(None)] {
        assert_eq!(code(&rig.ask(route, cx)), "not_found");
    }
    assert!(rig.recording());
    let busy = rig.start(AgentTarget::Fullscreen, 120, cx);
    assert_eq!(code(&busy), "invalid_arguments");
}

// A recording started under the user's frozen pick would film the overlay.
#[gpui::test]
async fn an_agent_recording_waits_for_no_capture_and_is_told_busy(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let claim = rig.coordinator.claim().unwrap();
    let busy = rig.start(AgentTarget::Fullscreen, 120, cx);
    assert_eq!(code(&busy), "invalid_arguments", "{busy}");
    assert_eq!(
        busy["error"]["hint"],
        "You're in the middle of a capture. Finish it or press Esc, and try again."
    );
    assert!(!rig.recording());
    drop(claim);
    rig.start(AgentTarget::Fullscreen, 120, cx);
    assert!(rig.recording());
    assert!(
        rig.coordinator.is_idle(),
        "the recording does not hold the screen"
    );
}

#[gpui::test]
async fn discard_deletes_the_recording(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.start(AgentTarget::Fullscreen, 120, cx);
    let gone = rig.ask(RecordRoute::Discard(None), cx);
    assert_eq!(
        (gone["ok"].clone(), gone["state"].clone()),
        (true.into(), "discarded".into()),
        "{gone}"
    );
    assert!(!rig.recording());
    assert_eq!(
        std::fs::read_dir(rig.paths.recordings_dir())
            .unwrap()
            .count(),
        0
    );
}

#[gpui::test]
async fn an_output_outside_the_scope_is_refused_before_anything_records(cx: &mut TestAppContext) {
    let rig = rig_for(
        AgentCfg {
            enabled: true,
            consent: es_model::Consent::AlwaysAllow,
            allowed_outputs: vec!["DP-2".into()],
            ..AgentCfg::default()
        },
        cx,
    );
    let output = |name: &str| AgentTarget::Output(OutputName::new(name).unwrap());
    let refused = rig.start(output("DP-1"), 120, cx);
    assert_eq!(code(&refused), "access_denied", "{refused}");
    assert!(!rig.recording());
    assert_eq!(rig.start(output("DP-2"), 120, cx)["ok"], true);
}

fn scoped(windows: &[&str], outputs: &[&str]) -> Scope {
    Scope::new(&AgentCfg {
        allowed_windows: windows.iter().map(|w| (*w).to_owned()).collect(),
        allowed_outputs: outputs.iter().map(|o| (*o).to_owned()).collect(),
        ..AgentCfg::default()
    })
}

#[test]
fn the_window_or_output_that_is_resolved_is_the_one_the_scope_is_checked_against() {
    let window = |id: &str| AgentTarget::Window(WindowSelector::id(id).unwrap());
    let only_mpv = scoped(&["mpv"], &[]);
    assert!(resolve(&window("2"), &only_mpv, &outputs(), &desk()).is_ok());
    let Err(Fail(code, why, ..)) = resolve(&window("1"), &only_mpv, &outputs(), &desk()) else {
        panic!("a foot window was recorded outside the scope");
    };
    assert_eq!(code, ErrorCode::AccessDenied, "{why}");
    let only_dp1 = scoped(&[], &["DP-1"]);
    assert!(
        resolve(&window("2"), &only_dp1, &outputs(), &desk()).is_err(),
        "a window on DP-2"
    );
    let focused = resolve(&AgentTarget::Fullscreen, &only_dp1, &outputs(), &desk());
    assert_eq!(
        focused.err().map(|f| f.0),
        Some(ErrorCode::AccessDenied),
        "the focused output is DP-2"
    );
    let geometry = AgentTarget::Geometry(Geometry::parse("10,10 100x100").unwrap());
    assert!(resolve(&geometry, &only_dp1, &outputs(), &desk()).is_ok());
    assert!(resolve(&geometry, &scoped(&["foot"], &[]), &outputs(), &desk()).is_err());
}

#[test]
fn targets_resolve_to_the_output_or_area_that_will_be_recorded() {
    let all = Scope::default();
    let chosen = |t: AgentTarget| resolve(&t, &all, &outputs(), &desk()).unwrap();
    let focused = chosen(AgentTarget::Fullscreen);
    assert!(matches!(focused.target, RecordingTarget::Display(OutputId(ref n)) if n == "DP-2"));
    let area = chosen(AgentTarget::Geometry(
        Geometry::parse("410,20 100x50").unwrap(),
    ));
    assert_eq!(area.area.map(|(o, _)| o.0), Some("DP-2".into()));
    let across = resolve(
        &AgentTarget::Geometry(Geometry::parse("350,0 100x50").unwrap()),
        &all,
        &outputs(),
        &desk(),
    );
    assert_eq!(across.err().map(|f| f.0), Some(ErrorCode::InvalidArguments));
    let missing = resolve(
        &AgentTarget::Output(OutputName::new("HDMI-1").unwrap()),
        &all,
        &outputs(),
        &desk(),
    );
    assert_eq!(missing.err().map(|f| f.0), Some(ErrorCode::NotFound));
}

#[test]
fn a_video_copied_to_the_videos_folder_is_reported_at_its_new_path() {
    let state = Rc::new(RefCell::new(State::default()));
    let (tx, rx) = async_channel::bounded(1);
    state.borrow_mut().active = Some(Active {
        id: "rec-1".into(),
        target: String::new(),
        max_seconds: 1,
        stopping: true,
        kept: None,
        waiters: vec![tx],
        _limit: Task::ready(()),
    });
    let card = uuid::Uuid::new_v4();
    let kept = Kept {
        card,
        file: "/r/screen.mkv".into(),
        duration_ms: 1500,
        width: 640,
        height: 360,
        saving: true,
    };
    ended_with(&state, &Ended::Kept(kept));
    assert!(rx.try_recv().is_err(), "the answer waits for the copy");
    ended_with(
        &state,
        &Ended::Saved {
            card,
            path: "/v/a.mp4".into(),
        },
    );
    let Ok(Outcome::Stopped(info)) = rx.try_recv() else {
        panic!("no answer after the copy");
    };
    assert_eq!(info.path.as_deref(), Some("/v/a.mp4"));
    assert_eq!(
        (info.duration_ms, info.width, info.height),
        (Some(1500), Some(640), Some(360))
    );
    assert_eq!(
        state.borrow().last.as_ref().map(|l| l.id.clone()),
        Some(Some("rec-1".into()))
    );
}

#[gpui::test]
async fn audio_and_options_reach_the_recording(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let spec = RecordSpec {
        audio: Some(Audio::Both),
        fps: Some(30),
        ..RecordSpec::new(AgentTarget::Fullscreen)
    };
    let request = cx.update(|cx| {
        rig.recordings.agent_request(
            RecordingTarget::Display(OutputId("DP-1".into())),
            spec.audio,
            cx,
        )
    });
    assert!(request.system_audio && request.mic.is_some());
    assert_eq!(request.delay_secs, 0, "an agent's recording starts at once");
    let none = cx.update(|cx| {
        rig.recordings
            .agent_request(RecordingTarget::Area, Some(Audio::None), cx)
    });
    assert!(!none.system_audio && none.mic.is_none());
    let started = rig.ask(RecordRoute::Start(spec), cx);
    assert_eq!(started["ok"], true, "{started}");
}

#[path = "agent_record_focus_tests.rs"]
mod focus;
