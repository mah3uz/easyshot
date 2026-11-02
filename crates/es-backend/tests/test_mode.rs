#![cfg(feature = "test-hooks")]

use es_backend::registry::Registry;
use es_backend::{
    Backend, Caps, Capture, Clipboard, Error, Probe, Provider, Result, RuleSet, TestConfig,
    VirtualInput, WindowList, WindowRules,
};
use es_geom::{Point, Rect};
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, Transform, WindowEvent,
    WindowId, WindowInfo,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const TEST_OUT: &str = "TEST-1";
const USER_OUT: &str = "DP-1";
const NO_CURSOR: CaptureOpts = CaptureOpts { cursor: false };

type Chan<T> = (async_channel::Sender<T>, async_channel::Receiver<T>);

// Providers are built on the test's own thread, so each test gets its own event sources.
thread_local! {
    static OUTPUT_EVENTS: Chan<OutputEvent> = async_channel::unbounded();
    static WINDOW_EVENTS: Chan<WindowEvent> = async_channel::unbounded();
}

fn output(name: &str) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical: Rect::new(0.0, 0.0, 10.0, 10.0),
        physical: (10, 10),
        scale: 1.0,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    }
}

fn window(id: &str, on: Option<&str>) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: "app".into(),
        title: id.into(),
        pid: None,
        logical: None,
        output: on.map(|o| OutputId(o.into())),
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

fn blank_frame(output: Option<OutputId>) -> Frame {
    Frame {
        output,
        size_px: (1, 1),
        logical: Rect::new(0.0, 0.0, 1.0, 1.0),
        scale: 1.0,
        bgra: vec![0u8; 4].into(),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    }
}

struct FakeCapture;

impl Capture for FakeCapture {
    fn outputs(&self) -> Vec<OutputInfo> {
        vec![output(USER_OUT), output(TEST_OUT)]
    }
    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        OUTPUT_EVENTS.with(|c| c.1.clone())
    }
    fn capture_outputs(&self, ids: &[OutputId], _o: CaptureOpts) -> Result<Vec<Frame>> {
        Ok(ids.iter().map(|id| blank_frame(Some(id.clone()))).collect())
    }
    fn capture_window(&self, _w: &WindowId, _o: CaptureOpts) -> Result<Frame> {
        Ok(blank_frame(None))
    }
}

struct FakeWindows;

impl WindowList for FakeWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        vec![
            window("mine", Some(TEST_OUT)),
            window("users", Some(USER_OUT)),
            window("unknown", None),
        ]
    }
    fn has_geometry(&self) -> bool {
        false
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        WINDOW_EVENTS.with(|c| c.1.clone())
    }
}

struct RealClipboard;

impl Clipboard for RealClipboard {
    fn set(&self, _items: Vec<ClipItem>) -> Result<()> {
        panic!("test mode must never reach the real clipboard");
    }
    fn needs_focused_window(&self) -> bool {
        false
    }
}

struct RealRules;

impl WindowRules for RealRules {
    fn install(&self, _rules: &RuleSet) -> Result<()> {
        panic!("test mode must never install rules");
    }
    fn remove(&self) -> Result<()> {
        panic!("test mode must never remove rules");
    }
}

struct RealInput;

impl VirtualInput for RealInput {
    fn scroll(&self, _at: Point, _dy: f64) -> Result<()> {
        panic!("test mode must never inject input");
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
    name: "real-clipboard",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(RealClipboard)),
}];
static RULES: &[Provider<dyn WindowRules>] = &[Provider {
    name: "real-rules",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(RealRules)),
}];
static INPUT: &[Provider<dyn VirtualInput>] = &[Provider {
    name: "real-input",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(RealInput)),
}];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("es-backend-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn isolated(dir: PathBuf) -> Backend {
    let probe = Probe {
        test: Some(TestConfig {
            output: OutputId(TEST_OUT.into()),
            dir,
        }),
        ..Probe::default()
    };
    let reg = Registry {
        capture: CAPTURE,
        windows: WINDOWS,
        clipboard: CLIPBOARD,
        rules: RULES,
        virtual_input: INPUT,
        ..Registry::EMPTY
    };
    Backend::detect_with(&probe, &reg)
}

fn ids(windows: &[WindowInfo]) -> Vec<&str> {
    windows.iter().map(|w| w.id.0.as_str()).collect()
}

#[test]
fn only_the_test_output_is_visible_or_capturable() {
    let b = isolated(scratch("outputs"));
    let outputs: Vec<_> = b.capture.outputs().into_iter().map(|o| o.id.0).collect();
    assert_eq!(outputs, [TEST_OUT]);
    let err = b
        .capture
        .capture_outputs(
            &[OutputId(TEST_OUT.into()), OutputId(USER_OUT.into())],
            NO_CURSOR,
        )
        .expect_err("the user's output must not be captured");
    assert!(matches!(err, Error::NotFound(ref o) if o == USER_OUT));
    assert_eq!(
        b.capture
            .capture_outputs(&[OutputId(TEST_OUT.into())], NO_CURSOR)
            .map(|f| f.len())
            .ok(),
        Some(1)
    );
}

#[test]
fn only_windows_on_the_test_output_are_listed_or_captured() {
    let b = isolated(scratch("windows"));
    assert_eq!(ids(&b.windows.snapshot()), ["mine"]);
    for hidden in ["users", "unknown"] {
        assert!(
            b.capture
                .capture_window(&WindowId(hidden.into()), NO_CURSOR)
                .is_err(),
            "{hidden} must not be capturable"
        );
    }
    assert!(
        b.capture
            .capture_window(&WindowId("mine".into()), NO_CURSOR)
            .is_ok()
    );
}

fn next<T>(rx: &async_channel::Receiver<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(e) = rx.try_recv() {
            return e;
        }
        assert!(Instant::now() < deadline, "no event within 5 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn events_for_other_outputs_and_their_windows_are_dropped() {
    let b = isolated(scratch("events"));
    let windows = b.windows.events();
    let outputs = b.capture.output_events();
    let tx = WINDOW_EVENTS.with(|c| c.0.clone());
    tx.send_blocking(WindowEvent::Opened(window("other", Some(USER_OUT))))
        .ok();
    tx.send_blocking(WindowEvent::Focused(WindowId("users".into())))
        .ok();
    tx.send_blocking(WindowEvent::Opened(window("new", Some(TEST_OUT))))
        .ok();
    assert_eq!(
        next(&windows),
        WindowEvent::Opened(window("new", Some(TEST_OUT)))
    );
    tx.send_blocking(WindowEvent::Changed(window("new", Some(USER_OUT))))
        .ok();
    assert_eq!(next(&windows), WindowEvent::Closed(WindowId("new".into())));

    let tx = OUTPUT_EVENTS.with(|c| c.0.clone());
    tx.send_blocking(OutputEvent::Added(output(USER_OUT))).ok();
    tx.send_blocking(OutputEvent::Removed(OutputId(USER_OUT.into())))
        .ok();
    tx.send_blocking(OutputEvent::Changed(output(TEST_OUT)))
        .ok();
    assert_eq!(next(&outputs), OutputEvent::Changed(output(TEST_OUT)));
}

#[test]
fn pointer_comes_only_from_set_pointer() {
    let b = isolated(scratch("pointer"));
    let pointer = b.pointer.as_ref().expect("test pointer");
    assert_eq!(pointer.pointer_global(), None);
    assert_eq!(pointer.output_under_pointer(), None);
    b.test
        .as_ref()
        .expect("hooks")
        .set_pointer(Point::new(3.0, 4.0));
    assert_eq!(pointer.pointer_global(), Some(Point::new(3.0, 4.0)));
    assert_eq!(
        pointer.output_under_pointer(),
        Some(OutputId(TEST_OUT.into()))
    );
    assert_eq!(pointer.focused_output(), Some(OutputId(TEST_OUT.into())));
    assert!(b.caps.contains(Caps::POINTER_GLOBAL));
}

#[test]
fn clipboard_writes_files_instead_of_the_real_clipboard() {
    let dir = scratch("clipboard");
    let b = isolated(dir.clone());
    b.clipboard
        .set(vec![
            ClipItem::Png(vec![0x89, b'P', b'N', b'G'].into()),
            ClipItem::Text("hello".into()),
        ])
        .expect("sink");
    let sink = dir.join("clipboard");
    assert_eq!(
        std::fs::read(sink.join("000001-0.png")).ok(),
        Some(vec![0x89, b'P', b'N', b'G'])
    );
    assert_eq!(
        std::fs::read_to_string(sink.join("000001-1.txt"))
            .ok()
            .as_deref(),
        Some("hello")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rules_only_log_and_virtual_input_is_off() {
    let b = isolated(scratch("rules"));
    let rules = b.rules.as_ref().expect("rules stay present");
    rules.install(&RuleSet::default()).expect("logged");
    rules.remove().expect("noop");
    assert!(b.virtual_input.is_none());
    assert!(!b.caps.contains(Caps::VIRTUAL_INPUT));
}

#[test]
fn test_mode_without_an_output_refuses_to_start() {
    let env = es_backend::Env::from_vars([("EASYSHOT_TEST", "1")]);
    assert!(matches!(
        TestConfig::from_env(&env),
        Err(es_backend::BackendError::NoTestOutput)
    ));
    let env = es_backend::Env::from_vars([("EASYSHOT_TEST", "0"), ("EASYSHOT_TEST_OUTPUT", "X")]);
    assert!(matches!(TestConfig::from_env(&env), Ok(None)));
}
