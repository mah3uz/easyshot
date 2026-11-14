use super::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

// The user is working: an agent must never move their focus, and never get their screen instead of
// the window it asked for.

#[derive(Default)]
struct Away {
    focus_calls: AtomicUsize,
}

fn mail() -> WindowInfo {
    WindowInfo {
        logical: None,
        output: Some(OutputId("DP-2".into())),
        ..window("9", "mail", "inbox")
    }
}

impl WindowList for Away {
    fn snapshot(&self) -> Vec<WindowInfo> {
        desk().0
    }

    fn has_geometry(&self) -> bool {
        true
    }

    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        async_channel::unbounded().1
    }

    fn all(&self) -> Vec<WindowInfo> {
        let mut all = desk().0;
        all.push(mail());
        all
    }

    fn can_focus(&self) -> bool {
        true
    }

    fn focus(&self, _: &WindowId) -> es_backend::Result<()> {
        self.focus_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Default)]
struct Alone {
    fails: bool,
    log: Mutex<Vec<String>>,
}

impl Alone {
    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl Capture for Alone {
    fn outputs(&self) -> Vec<OutputInfo> {
        Screens.outputs()
    }

    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        async_channel::unbounded().1
    }

    fn capture_outputs(&self, ids: &[OutputId], o: CaptureOpts) -> es_backend::Result<Vec<Frame>> {
        self.log
            .lock()
            .unwrap()
            .push(format!("output {}", ids[0].0));
        Screens.capture_outputs(ids, o)
    }

    fn capture_window(&self, w: &WindowId, _: CaptureOpts) -> es_backend::Result<Frame> {
        self.log.lock().unwrap().push(format!("window {}", w.0));
        if self.fails {
            return Err(es_backend::Error::Unsupported);
        }
        Ok(Frame {
            output: None,
            size_px: (30, 20),
            logical: Rect::new(0., 0., 30., 20.),
            scale: 1.0,
            bgra: vec![7; 30 * 20 * 4].into(),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        })
    }
}

fn inbox() -> AgentTarget {
    AgentTarget::Window(WindowSelector::id("9").unwrap())
}

#[test]
fn a_window_no_screen_shows_is_captured_alone_without_moving_focus() {
    let (capture, windows) = (Alone::default(), Away::default());
    let shot = grab(&capture, &windows, Caps::all(), &inbox(), &Scope::default()).unwrap();
    assert_eq!(shot.frame.size_px, (30, 20));
    assert_eq!(shot.window.map(|w| w.app_id), Some("mail".into()));
    assert_eq!(capture.log(), ["window 9"], "its own frame, no output");
    assert_eq!(windows.focus_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn without_window_capture_a_window_no_screen_shows_is_unsupported_and_nothing_is_cut() {
    let (capture, windows) = (Alone::default(), Away::default());
    let caps = Caps::all() - Caps::CAPTURE_WINDOW;
    let Err(Fail(code, _, hint)) = grab(&capture, &windows, caps, &inbox(), &Scope::default())
    else {
        panic!("a window no screen shows was captured without window capture");
    };
    assert_eq!(code, ErrorCode::Unsupported);
    assert!(hint.unwrap().contains("only while it is on screen"));
    assert!(capture.log().is_empty(), "{:?}", capture.log());
    assert_eq!(windows.focus_calls.load(Ordering::SeqCst), 0);

    let failing = Alone {
        fails: true,
        ..Alone::default()
    };
    let refused = grab(&failing, &windows, Caps::all(), &inbox(), &Scope::default());
    assert_eq!(refused.err().map(|f| f.0), Some(ErrorCode::Unsupported));
    assert_eq!(
        failing.log(),
        ["window 9"],
        "its output shows another window, so it is never cut from there"
    );
    assert_eq!(windows.focus_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_window_no_screen_shows_outside_the_scope_is_refused_before_any_capture() {
    let only = |windows: &[&str], outputs: &[&str]| {
        Scope::new(&AgentCfg {
            allowed_windows: windows.iter().map(|w| (*w).to_owned()).collect(),
            allowed_outputs: outputs.iter().map(|o| (*o).to_owned()).collect(),
            ..AgentCfg::default()
        })
    };
    let (capture, windows) = (Alone::default(), Away::default());
    for scope in [only(&["foot"], &[]), only(&[], &["DP-1"])] {
        let refused = grab(&capture, &windows, Caps::all(), &inbox(), &scope);
        assert_eq!(refused.err().map(|f| f.0), Some(ErrorCode::AccessDenied));
    }
    assert!(capture.log().is_empty(), "{:?}", capture.log());
    assert!(
        grab(
            &capture,
            &windows,
            Caps::all(),
            &inbox(),
            &only(&["mail"], &["DP-2"])
        )
        .is_ok()
    );
}
