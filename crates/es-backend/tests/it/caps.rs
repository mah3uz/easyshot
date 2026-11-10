use es_backend::registry::Registry;
use es_backend::{
    Backend, Caps, Capture, Clipboard, PointerQuery, Probe, Provider, Result, WindowList, WlGlobals,
};
use es_geom::{Point, Rect};
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, Transform, WindowEvent,
    WindowId, WindowInfo,
};
use std::time::Instant;

struct FakeCapture;

impl Capture for FakeCapture {
    fn outputs(&self) -> Vec<OutputInfo> {
        Vec::new()
    }
    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        async_channel::unbounded().1
    }
    fn capture_outputs(&self, _ids: &[OutputId], _o: CaptureOpts) -> Result<Vec<Frame>> {
        Ok(Vec::new())
    }
    fn capture_window(&self, _w: &WindowId, _o: CaptureOpts) -> Result<Frame> {
        Ok(Frame {
            output: None,
            size_px: (200, 100),
            logical: Rect::new(0.0, 0.0, 200.0, 100.0),
            scale: 1.0,
            bgra: vec![0u8; 200 * 100 * 4].into(),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        })
    }
}

struct FakeWindows;

impl WindowList for FakeWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        vec![WindowInfo {
            id: WindowId("w1".into()),
            app_id: "app".into(),
            title: "title".into(),
            pid: None,
            logical: Some(Rect::new(10.0, 20.0, 110.0, 70.0)),
            output: None,
            workspace: None,
            floating: false,
            focus_rank: None,
        }]
    }
    fn has_geometry(&self) -> bool {
        true
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        async_channel::unbounded().1
    }
    fn can_focus(&self) -> bool {
        true
    }
}

struct FakeClipboard;

impl Clipboard for FakeClipboard {
    fn set(&self, _items: Vec<ClipItem>) -> Result<()> {
        Ok(())
    }
    fn needs_focused_window(&self) -> bool {
        false
    }
}

struct FakePointer;

impl PointerQuery for FakePointer {
    fn pointer_global(&self) -> Option<Point> {
        None
    }
    fn output_under_pointer(&self) -> Option<OutputId> {
        None
    }
    fn focused_output(&self) -> Option<OutputId> {
        None
    }
}

static CAPTURE: &[Provider<dyn Capture>] = &[Provider {
    name: "fake-capture",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(FakeCapture)),
}];
static WINDOWS: &[Provider<dyn WindowList>] = &[Provider {
    name: "fake-windows",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(FakeWindows)),
}];
static CLIPBOARD: &[Provider<dyn Clipboard>] = &[Provider {
    name: "fake-clipboard",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(FakeClipboard)),
}];
static POINTER: &[Provider<dyn PointerQuery>] = &[Provider {
    name: "fake-pointer",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(FakePointer)),
}];

fn fakes() -> Registry {
    Registry {
        capture: CAPTURE,
        windows: WINDOWS,
        clipboard: CLIPBOARD,
        pointer: POINTER,
        ..Registry::EMPTY
    }
}

fn probe() -> Probe {
    let mut p = Probe {
        globals: WlGlobals {
            layer_shell: Some(4),
            ..WlGlobals::default()
        },
        ..Probe::default()
    };
    p.tools.tesseract = true;
    p
}

#[test]
fn caps_are_exactly_what_the_chosen_providers_and_tools_give() {
    let b = Backend::detect_with(&probe(), &fakes());
    assert_eq!(
        b.caps(),
        Caps::LAYER_SHELL
            | Caps::CAPTURE_OUTPUT
            | Caps::WINDOW_LIST
            | Caps::WINDOW_GEOMETRY
            | Caps::FOCUS_WINDOW
            | Caps::CLIPBOARD_NO_FOCUS
            | Caps::POINTER_GLOBAL
            | Caps::OCR
    );
}

#[test]
fn nothing_detected_gives_no_caps_and_working_fallbacks() {
    let b = Backend::detect_with(&Probe::default(), &Registry::EMPTY);
    assert_eq!(b.caps(), Caps::empty());
    assert!(b.capture.outputs().is_empty());
    assert!(b.windows.snapshot().is_empty());
    assert!(b.clipboard.set(vec![ClipItem::Text("x".into())]).is_err());
    assert!(b.shortcuts().user_binds_required());
    assert_eq!(
        b.report().missing_tools.len(),
        es_backend::TOOL_PACKAGES.len()
    );
}

#[test]
fn window_frames_take_the_window_list_geometry() {
    let b = Backend::detect_with(&probe(), &fakes());
    let f = b
        .capture
        .capture_window(&WindowId("w1".into()), CaptureOpts { cursor: false })
        .expect("fake capture");
    assert_eq!(f.logical, Rect::new(10.0, 20.0, 110.0, 70.0));
    assert_eq!(f.scale, 2.0);
}

#[test]
fn doctor_lists_providers_caps_and_missing_tools() {
    let b = Backend::detect_with(&probe(), &fakes());
    let v: serde_json::Value = serde_json::from_str(&b.doctor_json()).expect("valid JSON");
    let capture = v["providers"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["capability"] == "capture"))
        .expect("capture entry");
    assert_eq!(capture["chosen"], "fake-capture");
    assert!(
        v["caps"]
            .as_array()
            .is_some_and(|c| c.iter().any(|x| x == "OCR"))
    );
    let missing = v["missing_tools"].as_array().expect("missing tools");
    assert!(
        missing
            .iter()
            .any(|t| t["tool"] == "wf-recorder" && t["package"] == "wf-recorder")
    );
    assert!(!missing.iter().any(|t| t["tool"] == "tesseract"));
}

#[test]
fn doctor_from_the_detected_backend_lists_wayland_globals_and_portals() {
    let mut p = probe();
    p.globals.all = vec![("zwlr_layer_shell_v1".into(), 4)];
    p.portals.file_chooser = true;
    let b = Backend::detect_with(&p, &fakes());
    let v: serde_json::Value = serde_json::from_str(&b.doctor_json()).expect("valid JSON");
    assert_eq!(v["wayland_globals"]["zwlr_layer_shell_v1"], 4);
    assert_eq!(v["portals"]["FileChooser"], true);
    assert_eq!(v["portals"]["ScreenCast"], false);
}

// Runs inside tests/harness/sway.sh, whose only output is HEADLESS-1; skipped anywhere else.
#[test]
fn platform_detect_picks_ext_capture_in_headless_sway() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let probe = Probe::gather().expect("probe");
    let b = Backend::detect(&probe);
    assert_eq!(
        b.report().chosen("capture"),
        Some("ext-image-copy"),
        "{}",
        b.doctor_json()
    );
    assert!(b.caps().contains(Caps::CAPTURE_OUTPUT | Caps::LAYER_SHELL));
}

#[test]
fn a_usable_binds_file_sets_shortcut_triggers_and_not_the_portal_flag() {
    let p = Probe {
        env: es_backend::Env::from_vars([(es_backend::wlroots::SOCKET_ENV, "/nonexistent/sock")]),
        portals: es_backend::PortalIfaces {
            global_shortcuts: true,
            ..Default::default()
        },
        ..Probe::default()
    };
    let reg = Registry {
        binds: es_backend::registry::BINDS,
        shortcuts: es_backend::registry::SHORTCUTS,
        ..Registry::EMPTY
    };
    let b = Backend::detect_with(&p, &reg);
    assert!(b.caps().contains(Caps::SHORTCUT_TRIGGERS));
    assert!(!b.caps().contains(Caps::PORTAL_SHORTCUTS));
    assert!(b.shortcuts().user_binds_required());
}
