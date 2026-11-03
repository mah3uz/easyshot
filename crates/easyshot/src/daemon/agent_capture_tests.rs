use super::*;
use crate::daemon::agent::gate::tests::{GateRig, ME};
use es_app::events::Origin;
use es_app::jobs::{JobId, Jobs};
use es_model::AgentCfg;
use es_model::platform::{CaptureOpts, OutputEvent, OutputInfo, Transform, WindowEvent, WindowId};
use es_protocol::agent::{Geometry, OutputName};
use es_protocol::{AgentRoute, JobStatus};
use gpui::TestAppContext;
use std::cell::Cell;
use std::time::Instant;

fn output(name: &str, logical: Rect) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical,
        physical: (logical.width() as u32 * 2, logical.height() as u32 * 2),
        scale: 2.0,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    }
}

struct Screens;

impl Capture for Screens {
    fn outputs(&self) -> Vec<OutputInfo> {
        vec![
            output("DP-1", Rect::new(0., 0., 400., 300.)),
            output("DP-2", Rect::new(400., 0., 800., 300.)),
        ]
    }

    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        async_channel::unbounded().1
    }

    fn capture_outputs(&self, ids: &[OutputId], _: CaptureOpts) -> es_backend::Result<Vec<Frame>> {
        let info = self
            .outputs()
            .into_iter()
            .find(|o| o.id == ids[0])
            .ok_or(es_backend::Error::Unsupported)?;
        let size_px = info.physical;
        Ok(vec![Frame {
            output: Some(info.id),
            size_px,
            logical: info.logical,
            scale: info.scale,
            bgra: vec![0; (size_px.0 * size_px.1 * 4) as usize].into(),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        }])
    }

    fn capture_window(&self, _: &WindowId, _: CaptureOpts) -> es_backend::Result<Frame> {
        Err(es_backend::Error::Unsupported)
    }
}

fn window(id: &str, app_id: &str, title: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: app_id.into(),
        title: title.into(),
        pid: None,
        logical: Some(Rect::new(100., 50., 300., 150.)),
        output: Some(OutputId("DP-1".into())),
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

struct Windows(Vec<WindowInfo>);

impl WindowList for Windows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        self.0.clone()
    }

    fn has_geometry(&self) -> bool {
        true
    }

    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        async_channel::unbounded().1
    }
}

fn desk() -> Windows {
    Windows(vec![
        window("1", "foot", "build"),
        window("2", "foot", "logs"),
        window("3", "mpv", "foot"),
        window("4", "firefox", "2"),
    ])
}

fn shot(target: &AgentTarget) -> Result<Shot, Fail> {
    grab(&Screens, &desk(), target, &Scope::default())
}

fn selector(text: &str) -> WindowSelector {
    WindowSelector::any(text).unwrap()
}

#[test]
fn a_window_name_is_an_id_first_then_an_exact_app_id_then_an_exact_title() {
    let windows = desk().0;
    let pick = |text: &str| resolve_window(&windows, &selector(text)).map(|w| w.id.0);
    assert_eq!(pick("3"), Ok("3".into()), "an id");
    assert_eq!(
        pick("2"),
        Ok("2".into()),
        "an id beats the title of window 4"
    );
    assert_eq!(pick("mpv"), Ok("3".into()), "an app id");
    assert_eq!(pick("build"), Ok("1".into()), "a title");
    assert_eq!(pick("logs"), Ok("2".into()));
    assert_eq!(
        pick("fire").unwrap_err().0,
        ErrorCode::NotFound,
        "exact only"
    );
    assert_eq!(pick("BUILD").unwrap_err().0, ErrorCode::NotFound);
}

#[test]
fn a_name_that_fits_two_windows_is_refused_with_the_candidates_listed() {
    let windows = desk().0;
    let Fail(code, message) = resolve_window(&windows, &selector("foot")).unwrap_err();
    assert_eq!(code, ErrorCode::InvalidArguments);
    assert!(message.contains("2 windows"), "{message}");
    assert!(message.contains("1 (foot, \"build\")"), "{message}");
    assert!(message.contains("2 (foot, \"logs\")"), "{message}");
    assert!(!message.contains("mpv"), "{message}");
}

#[test]
fn an_explicit_kind_of_selector_looks_only_at_its_own_field() {
    let windows = desk().0;
    let by = |s: WindowSelector| resolve_window(&windows, &s).map(|w| w.id.0);
    assert_eq!(by(WindowSelector::title("foot").unwrap()), Ok("3".into()));
    assert_eq!(
        by(WindowSelector::app_id("foot").unwrap()).unwrap_err().0,
        ErrorCode::InvalidArguments
    );
    assert_eq!(
        by(WindowSelector::id("foot").unwrap()).unwrap_err().0,
        ErrorCode::NotFound
    );
}

#[test]
fn a_geometry_is_cut_from_the_output_that_holds_it_in_that_outputs_pixels() {
    let shot = shot(&AgentTarget::Geometry(
        Geometry::parse("410,20 100x50").unwrap(),
    ))
    .unwrap();
    assert_eq!(shot.frame.size_px, (200, 100), "logical 100x50 at scale 2");
    assert_eq!(shot.frame.scale, 2.0);
    assert_eq!(shot.frame.output, Some(OutputId("DP-2".into())));
    assert_eq!(shot.kind, StoredKind::Region);
}

#[test]
fn a_geometry_across_two_outputs_or_off_every_output_is_refused() {
    let across = shot(&AgentTarget::Geometry(
        Geometry::parse("350,0 100x50").unwrap(),
    ));
    assert_eq!(across.err().map(|f| f.0), Some(ErrorCode::InvalidArguments));
    let off = shot(&AgentTarget::Geometry(
        Geometry::parse("900,0 10x10").unwrap(),
    ));
    assert_eq!(off.err().map(|f| f.0), Some(ErrorCode::NotFound));
}

#[test]
fn an_output_is_captured_whole_and_an_unknown_one_lists_the_real_ones() {
    let shot_of = |name: &str| shot(&AgentTarget::Output(OutputName::new(name).unwrap()));
    let whole = shot_of("DP-1").unwrap();
    assert_eq!(whole.frame.size_px, (800, 600));
    assert_eq!(whole.kind, StoredKind::Fullscreen);
    let Fail(code, message) = shot_of("HDMI-A-1").err().unwrap();
    assert_eq!(code, ErrorCode::NotFound);
    assert!(message.contains("DP-1, DP-2"), "{message}");
}

#[test]
fn a_window_is_cut_from_its_output_when_the_compositor_cannot_capture_it_alone() {
    let shot = shot(&AgentTarget::Window(selector("build"))).unwrap();
    assert_eq!(shot.frame.size_px, (400, 200), "logical 200x100 at scale 2");
    assert_eq!(shot.kind, StoredKind::Window);
    assert_eq!(shot.window.map(|w| w.id.0), Some("1".into()));
}

#[test]
fn a_window_that_is_not_there_is_not_found_and_captures_nothing() {
    let missing = shot(&AgentTarget::Window(selector("nothing")));
    assert_eq!(missing.err().map(|f| f.0), Some(ErrorCode::NotFound));
}

#[test]
fn a_desktop_without_the_capability_gets_unsupported_not_a_failure() {
    let window = AgentTarget::Window(selector("1"));
    let err = check_caps(Caps::CAPTURE_OUTPUT, &window).unwrap_err();
    assert_eq!(err.0, ErrorCode::Unsupported);
    assert!(check_caps(Caps::WINDOW_LIST, &window).is_ok());
    let err = check_caps(Caps::WINDOW_LIST, &AgentTarget::Fullscreen).unwrap_err();
    assert_eq!(err.0, ErrorCode::Unsupported);
    assert!(check_caps(Caps::CAPTURE_OUTPUT, &AgentTarget::Fullscreen).is_ok());
}

#[test]
fn an_out_path_needs_an_existing_folder_and_is_not_one() {
    let dir = tempfile::tempdir().unwrap();
    assert!(check_out(&dir.path().join("a.png")).is_ok());
    let missing = check_out(&dir.path().join("nope/a.png")).unwrap_err();
    assert_eq!(missing.0, ErrorCode::InvalidArguments);
    std::fs::create_dir(dir.path().join("taken.png")).unwrap();
    let folder = check_out(&dir.path().join("taken.png")).unwrap_err();
    assert_eq!(folder.0, ErrorCode::InvalidArguments);
}

fn ask(
    capture: Option<CaptureRunner>,
    origin: Origin,
    cx: &mut TestAppContext,
) -> serde_json::Value {
    let (jobs, finished) = Jobs::new();
    let sources = super::super::Sources {
        caps: Caps::all(),
        outputs: Arc::new(Vec::new),
        windows: Arc::new(Vec::new),
    };
    let rig = GateRig::permissive(cx);
    rig.gate.callers().borrow_mut().insert(
        JobId(3),
        es_desktop::dbus::Caller {
            uid: Some(ME),
            pid: None,
        },
    );
    let handler = super::super::handler_with(sources, jobs, rig.gate.clone(), capture, None);
    let route = AgentRoute::Capture(CaptureSpec::new(AgentTarget::Fullscreen));
    cx.update(|cx| handler(JobId(3), route, origin, cx));
    cx.run_until_parked();
    let done = finished.try_recv().expect("the job finishes");
    assert_eq!(
        done.status == JobStatus::Ok,
        done.detail
            .as_deref()
            .is_some_and(|d| d.contains("\"ok\":true"))
    );
    serde_json::from_str(done.detail.as_deref().unwrap()).unwrap()
}

#[gpui::test]
fn the_capture_route_runs_the_runner_only_for_a_request_the_gate_lets_in(cx: &mut TestAppContext) {
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let runner: CaptureRunner = Rc::new(move |_, _, cx| {
        counted.set(counted.get() + 1);
        cx.spawn(async move |_| AgentReply {
            capture: Some(Box::new(CaptureInfo {
                path: "/x.png".into(),
                width: 1,
                height: 1,
                scale: 1.0,
                output: None,
                window: None,
            })),
            ..AgentReply::ok(ReplyKind::Capture)
        })
    });
    let reply = ask(Some(runner.clone()), Origin::DBus, cx);
    assert_eq!(
        (reply["kind"].as_str(), reply["path"].as_str()),
        (Some("capture"), Some("/x.png"))
    );
    assert_eq!(runs.get(), 1);
    let refused = ask(Some(runner), Origin::Url, cx);
    assert_eq!(refused["error"]["code"], "access_denied");
    assert_eq!(runs.get(), 1, "a refused request starts no capture");
}

#[gpui::test]
fn without_a_capture_runner_the_route_answers_unsupported(cx: &mut TestAppContext) {
    let reply = ask(None, Origin::DBus, cx);
    assert_eq!(reply["error"]["code"], "unsupported");
}

#[test]
fn a_window_or_output_outside_the_scope_is_refused_where_the_capture_resolves_it() {
    let only = |windows: &[&str], outputs: &[&str]| {
        Scope::new(&AgentCfg {
            allowed_windows: windows.iter().map(|w| (*w).to_owned()).collect(),
            allowed_outputs: outputs.iter().map(|o| (*o).to_owned()).collect(),
            ..AgentCfg::default()
        })
    };
    let by_id = |id: &str| AgentTarget::Window(WindowSelector::id(id).unwrap());
    let mpv = only(&["mpv"], &[]);
    assert!(grab(&Screens, &desk(), &by_id("3"), &mpv).is_ok());
    let Err(Fail(code, message)) = grab(&Screens, &desk(), &by_id("4"), &mpv) else {
        panic!("a firefox window the gate never saw was captured outside the scope");
    };
    assert_eq!(code, ErrorCode::AccessDenied, "{message}");
    let other = Scope::new(&AgentCfg {
        allowed_outputs: vec!["NOT-THERE".into()],
        ..AgentCfg::default()
    });
    for target in [
        AgentTarget::Output(OutputName::new(&Screens.outputs()[0].id.0).unwrap()),
        by_id("3"),
    ] {
        let refused = grab(&Screens, &desk(), &target, &other).err().map(|f| f.0);
        assert_eq!(refused, Some(ErrorCode::AccessDenied), "{target:?}");
    }
}
