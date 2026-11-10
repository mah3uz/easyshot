use crate::window_mode_support;

use es_geom::{Point, Rect, Size};
use es_model::platform::{OutputId, OutputInfo, Transform, WindowId, WindowInfo};
use es_overlays::record::bar::{BAR, Popup};
use es_overlays::record::control::{BAR as CONTROL, CONFIRM, TIP_ROOM};
use es_overlays::record::options::WIDTH;
use es_overlays::record::{
    BarEvent, CaptureBar, Confirm, ControlBar, ControlEvent, OptionsEvent, Phase, Placement,
    Recorded, RecordingDefaults, RecordingOptions, RecordingTarget, control_bar, highlight_strips,
};
use es_protocol::{Route, TimedMode};
use es_record::{AudioDevice, AudioKind, Clock};
use es_ui::tokens::ThemeMode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use window_mode_support::{Surface, rgb};

#[derive(Default)]
struct TestClock(Mutex<Duration>);

impl TestClock {
    fn advance(&self, secs: u64) {
        *self.0.lock().expect("clock") += Duration::from_secs(secs);
    }
}

impl Clock for TestClock {
    fn now(&self) -> Duration {
        *self.0.lock().expect("clock")
    }
    fn sleep(&self, d: Duration) {
        *self.0.lock().expect("clock") += d;
    }
    fn monotonic_us(&self) -> u64 {
        self.now().as_micros() as u64
    }
}

fn output(name: &str, x0: f64, w: f64, h: f64) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: name.into(),
        logical: Rect::new(x0, 0., x0 + w, h),
        physical: ((w * 1.5) as u32, (h * 1.5) as u32),
        scale: 1.5,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    }
}

fn devices() -> Vec<AudioDevice> {
    vec![
        AudioDevice {
            name: "alsa_output.monitor".into(),
            description: "Speakers".into(),
            kind: AudioKind::SystemOutput,
        },
        AudioDevice {
            name: "alsa_input.usb".into(),
            description: "USB Mic".into(),
            kind: AudioKind::Microphone,
        },
    ]
}

fn window(title: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId("w1".into()),
        app_id: "foot".into(),
        title: title.into(),
        pid: None,
        logical: Some(Rect::new(10., 10., 400., 300.)),
        output: None,
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

// Captioned items are 72 wide with 4 between them, after the 1-point border and 6 of padding.
fn item_x(i: usize) -> f32 {
    7. + i as f32 * 76. + 36.
}
const TIMER_X: f32 = 514.;
const RECORDING_X: f32 = 590.;
const CLOSE_X: f32 = 648.;

#[test]
fn every_capture_bar_item_emits_its_route() {
    let mut s = Surface::open(BAR.width as f32, BAR.height as f32, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), false, cx)
    });
    s.app.screenshot("capture-bar");
    let view = s.view.clone();
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(ThemeMode::Light, cx);
        view.update(cx, |_, cx| cx.notify());
    });
    s.app.screenshot("capture-bar-light");
    let (events, _sub) = s.record::<BarEvent>();
    for i in 0..6 {
        s.click(item_x(i), 32.);
    }
    s.click(CLOSE_X, 32.);
    let routes: Vec<String> = events.borrow().iter().map(|e| format!("{e:?}")).collect();
    assert_eq!(
        routes,
        [
            Route::CaptureRegion,
            Route::CaptureFullscreen,
            Route::CaptureWindow,
            Route::CaptureScroll,
            Route::Ocr,
            Route::ColorPicker,
        ]
        .iter()
        .map(|r| format!("{:?}", BarEvent::Route(r.clone())))
        .chain(["Close".to_string()])
        .collect::<Vec<_>>()
    );
}

#[test]
fn escape_closes_a_fresh_capture_bar_without_a_click() {
    let mut s = Surface::open(BAR.width as f32, BAR.height as f32, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), false, cx)
    });
    let (events, _sub) = s.record::<BarEvent>();
    s.key("escape");
    assert_eq!(
        events
            .borrow()
            .iter()
            .map(|e| format!("{e:?}"))
            .collect::<Vec<_>>(),
        ["Close"],
        "the bar takes keyboard focus when it opens, so Esc reaches it"
    );
}

#[test]
fn timer_and_recording_open_their_popups_above_the_bar() {
    let h = 480.;
    let mut s = Surface::open(BAR.width as f32, h, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), false, cx)
    });
    let y = h - 32.;
    let view = s.view.clone();
    s.click(TIMER_X, y);
    assert_eq!(
        s.app.cx.update(|cx| view.read(cx).popup()),
        Some(Popup::Timer)
    );
    s.app.screenshot("capture-bar-timer");
    s.click(RECORDING_X, y);
    assert_eq!(
        s.app.cx.update(|cx| view.read(cx).popup()),
        Some(Popup::Options)
    );
    let size = s.app.cx.update(|cx| view.read(cx).size(cx));
    assert!(size.height > BAR.height + 150., "{size:?}");
    s.app.screenshot("capture-bar-options");
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(ThemeMode::Light, cx);
        view.update(cx, |bar, cx| {
            bar.options().update(cx, |_, cx| cx.notify());
            cx.notify();
        });
    });
    s.app.screenshot("capture-bar-options-light");
    s.key("escape");
    assert_eq!(s.app.cx.update(|cx| view.read(cx).popup()), None);
}

// Source buttons: 72 wide, 8 apart, centred in the 418 points inside the padding.
const ROW_Y: f32 = 97.;
const AREA_X: f32 = 60.5;
const DISPLAY_X: f32 = 140.5;
const WINDOW_X: f32 = 220.5;
const MIC_X: f32 = 319.5;
const AUDIO_X: f32 = 399.5;
const MORE_X: f32 = 427.;
const LESS_X: f32 = 355.;
const STEPPER_Y: f32 = 153.;

fn menu_row(i: usize) -> f32 {
    144. + 13. + i as f32 * 26.
}

#[test]
fn the_options_build_a_request_from_their_controls() {
    let outputs = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let mut s = Surface::open(WIDTH as f32, 360., |_| {
        RecordingOptions::new(outputs, devices())
    });
    let (events, _sub) = s.record::<OptionsEvent>();
    s.click(MIC_X, ROW_Y);
    s.app.screenshot("recording-options-mic-menu");
    s.click(LESS_X + 40., menu_row(1));
    s.click(AUDIO_X, ROW_Y);
    for _ in 0..4 {
        s.click(MORE_X, STEPPER_Y);
    }
    s.click(LESS_X, STEPPER_Y);
    s.click(DISPLAY_X, ROW_Y);
    s.app.screenshot("recording-options-display-menu");
    s.click(LESS_X, menu_row(1));
    let starts: Vec<_> = events
        .borrow()
        .iter()
        .filter_map(|e| match e {
            OptionsEvent::Start(r) => Some(r.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 1, "one start, from the display menu");
    let request = &starts[0];
    assert!(matches!(&request.target, RecordingTarget::Display(id) if id.0 == "B"));
    assert_eq!(request.mic.as_deref(), Some("alsa_input.usb"));
    assert!(request.system_audio);
    assert_eq!(request.delay_secs, 3);

    s.click(AREA_X, ROW_Y);
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::Start(r)) if matches!(r.target, RecordingTarget::Area)
    ));
    let view = s.view.clone();
    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.set_windows(vec![window("notes.md")], cx)));
    s.click(WINDOW_X, ROW_Y);
    s.app.screenshot("recording-options-window-menu");
    s.click(LESS_X, menu_row(1));
    assert!(
        matches!(events.borrow().last(), Some(OptionsEvent::RefreshWindows)),
        "the row after the windows refreshes them"
    );
    s.click(LESS_X, menu_row(0));
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::Start(r)) if matches!(&r.target, RecordingTarget::Window(w) if w.title == "notes.md")
    ));
}

#[test]
fn the_options_start_from_the_given_defaults_and_requests_carry_them() {
    let mut s = Surface::open(WIDTH as f32, 360., |_| {
        RecordingOptions::new(vec![output("A", 0., 1280., 720.)], devices())
    });
    let view = s.view.clone();
    s.app.cx.update(|cx| {
        view.update(cx, |o, cx| {
            o.set_defaults(
                RecordingDefaults {
                    mic: Some("default".into()),
                    system_audio: true,
                    delay_secs: 3,
                },
                cx,
            )
        })
    });
    let request = s
        .app
        .cx
        .update(|cx| view.read(cx).request(RecordingTarget::Area));
    assert_eq!(request.mic.as_deref(), Some("default"));
    assert!(request.system_audio);
    assert_eq!(request.delay_secs, 3);
}

#[test]
fn a_single_display_records_without_a_menu() {
    let mut s = Surface::open(WIDTH as f32, 360., |_| {
        RecordingOptions::new(vec![output("A", 0., 1280., 720.)], devices())
    });
    let (events, _sub) = s.record::<OptionsEvent>();
    s.click(DISPLAY_X, ROW_Y);
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::Start(r)) if matches!(&r.target, RecordingTarget::Display(id) if id.0 == "A")
    ));
    let view = s.view.clone();
    assert_eq!(s.app.cx.update(|cx| view.read(cx).menu()), None);
}

const GRIP: f32 = 18.;
const BAR_Y: f32 = TIP_ROOM as f32 + 19.;
const PAUSE_X: f32 = GRIP + 1. + 132. + 1. + 21.;
const RESTART_X: f32 = PAUSE_X + 43.;
const DISCARD_X: f32 = RESTART_X + 43.;
const STOP_X: f32 = GRIP + 60.;

#[test]
fn the_timer_leaves_out_paused_time() {
    let clock = Arc::new(TestClock::default());
    let c = clock.clone();
    let mut s = Surface::open(
        CONTROL.width as f32,
        (CONTROL.height + TIP_ROOM) as f32,
        move |cx| ControlBar::new(c, Vec::new(), cx),
    );
    let view = s.view.clone();
    let read = |s: &mut Surface<ControlBar>| {
        let v = view.clone();
        s.app.cx.update(|cx| {
            let bar = v.read(cx);
            (bar.phase(), bar.elapsed().as_secs())
        })
    };
    s.app.screenshot("control-bar-preparing");
    clock.advance(4);
    assert_eq!(
        read(&mut s),
        (Phase::Starting, 0),
        "no time before the first frame"
    );
    s.app.cx.update(|cx| view.update(cx, |b, cx| b.started(cx)));
    clock.advance(5);
    s.app
        .cx
        .update(|cx| view.update(cx, |b, cx| b.set_paused(true, cx)));
    clock.advance(10);
    assert_eq!(read(&mut s), (Phase::Paused, 5));
    s.app.screenshot("control-bar-paused");
    s.app
        .cx
        .update(|cx| view.update(cx, |b, cx| b.set_paused(false, cx)));
    clock.advance(2);
    assert_eq!(read(&mut s), (Phase::Recording, 7));
    s.app.cx.advance_clock(Duration::from_millis(300));
    s.app.cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
    let shot = s.app.screenshot("control-bar-recording");
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(ThemeMode::Light, cx);
        view.update(cx, |_, cx| cx.notify());
    });
    s.app.screenshot("control-bar-recording-light");
    let red = rgb(&shot, 18. + GRIP, BAR_Y);
    assert!(
        i32::from(red[0]) > i32::from(red[2]) + 60,
        "the stop glyph is red: {red:?}"
    );
}

#[test]
fn nothing_answers_while_starting_or_finishing() {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(
        CONTROL.width as f32,
        (CONTROL.height + TIP_ROOM) as f32,
        move |cx| ControlBar::new(clock, Vec::new(), cx),
    );
    let (events, _sub) = s.record::<ControlEvent>();
    for x in [STOP_X, PAUSE_X, RESTART_X, DISCARD_X] {
        s.click(x, BAR_Y);
    }
    assert!(events.borrow().is_empty(), "{:?}", events.borrow());
    let view = s.view.clone();
    s.app.cx.update(|cx| {
        view.update(cx, |b, cx| {
            b.started(cx);
            b.finishing(cx);
        })
    });
    s.app.screenshot("control-bar-saving");
    s.click(STOP_X, 19.);
    assert!(events.borrow().is_empty());
}

// The confirm card's buttons are 88 wide, 8 apart, inside 14 of padding.
const CANCEL: (f32, f32) = (185., 96. + TIP_ROOM as f32);
const OK: (f32, f32) = (281., 96. + TIP_ROOM as f32);
const BAR_IN_CONFIRM: (f32, f32) = (
    (CONFIRM.width as f32 - CONTROL.width as f32) / 2.,
    (CONFIRM.height + TIP_ROOM - CONTROL.height) as f32 + 19.,
);

fn confirm_surface() -> Surface<ControlBar> {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(
        CONFIRM.width as f32,
        (CONFIRM.height + TIP_ROOM) as f32,
        move |cx| ControlBar::new(clock, Vec::new(), cx),
    );
    let view = s.view.clone();
    s.app.cx.update(|cx| view.update(cx, |b, cx| b.started(cx)));
    s
}

fn confirm(s: &mut Surface<ControlBar>) -> Option<Confirm> {
    let v = s.view.clone();
    s.app.cx.update(|cx| v.read(cx).confirm())
}

#[test]
fn start_over_and_discard_ask_first() {
    let mut s = confirm_surface();
    let (events, _sub) = s.record::<ControlEvent>();
    s.click(STOP_X + BAR_IN_CONFIRM.0, BAR_IN_CONFIRM.1);
    s.click(PAUSE_X + BAR_IN_CONFIRM.0, BAR_IN_CONFIRM.1);
    assert_eq!(
        *events.borrow(),
        vec![
            ControlEvent::Route(Route::RecordStop),
            ControlEvent::Route(Route::RecordPause)
        ]
    );
    events.borrow_mut().clear();

    s.click(RESTART_X + BAR_IN_CONFIRM.0, BAR_IN_CONFIRM.1);
    assert_eq!(confirm(&mut s), Some(Confirm::Restart));
    s.app.screenshot("control-bar-confirm-restart");
    s.click(CANCEL.0, CANCEL.1);
    assert_eq!(confirm(&mut s), None);
    assert_eq!(
        *events.borrow(),
        vec![ControlEvent::Resized, ControlEvent::Resized]
    );
    s.click(RESTART_X + BAR_IN_CONFIRM.0, BAR_IN_CONFIRM.1);
    s.click(OK.0, OK.1);
    assert_eq!(
        events.borrow().last(),
        Some(&ControlEvent::Route(Route::RecordRestart))
    );

    let mut s = confirm_surface();
    let view = s.view.clone();
    let (events, _sub) = s.record::<ControlEvent>();
    s.click(DISCARD_X + BAR_IN_CONFIRM.0, BAR_IN_CONFIRM.1);
    assert_eq!(confirm(&mut s), Some(Confirm::Discard));
    let size = s.app.cx.update(|cx| view.read(cx).size());
    assert_eq!(size, Size::new(CONFIRM.width, CONFIRM.height + TIP_ROOM));
    s.app.screenshot("control-bar-confirm-discard");
    s.click(OK.0, OK.1);
    assert_eq!(
        events.borrow().last(),
        Some(&ControlEvent::Route(Route::RecordDiscard))
    );
    let routes = events
        .borrow()
        .iter()
        .filter(|e| matches!(e, ControlEvent::Route(_)))
        .count();
    assert_eq!(routes, 1, "only the confirm button acts");
}

fn screenshot_timer_row(h: f32, i: usize) -> f32 {
    h - 64. - 8. - 248. + 4. + 24. + 12. + i as f32 * 24.
}

#[test]
fn the_screenshot_timer_steps_off_3_5_10_and_times_area_screen_and_window() {
    let h = 480.;
    let mut s = Surface::open(BAR.width as f32, h, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), false, cx)
    });
    let y = h - 32.;
    let view = s.view.clone();
    let mut routes = Vec::new();
    for (row, secs) in [(1, 3), (2, 5), (3, 10), (0, 0)] {
        s.click(TIMER_X, y);
        s.click(BAR.width as f32 / 2., screenshot_timer_row(h, row));
        assert_eq!(s.app.cx.update(|cx| view.read(cx).screenshot_timer()), secs);
        assert_eq!(s.app.cx.update(|cx| view.read(cx).popup()), None);
        let (events, _sub) = s.record::<BarEvent>();
        for i in [0, 1, 2, 3] {
            s.click(item_x(i), y);
        }
        routes.extend(events.borrow().iter().filter_map(|e| match e {
            BarEvent::Route(r) => Some(r.clone()),
            _ => None,
        }));
    }
    let timed = |mode, secs| Route::timed(mode, Some(secs));
    let mut expected = Vec::new();
    for secs in [3, 5, 10] {
        expected.extend([
            timed(TimedMode::Region, secs),
            timed(TimedMode::Fullscreen, secs),
            timed(TimedMode::Window, secs),
            Route::CaptureScroll,
        ]);
    }
    expected.extend([
        Route::CaptureRegion,
        Route::CaptureFullscreen,
        Route::CaptureWindow,
        Route::CaptureScroll,
    ]);
    assert_eq!(routes, expected);
}

#[test]
fn shift_on_scroll_starts_a_region_and_nothing_else_changes() {
    let h = 480.;
    let mut s = Surface::open(BAR.width as f32, h, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), false, cx)
    });
    let (events, _sub) = s.record::<BarEvent>();
    let y = h - 32.;
    s.shift_click(item_x(3), y);
    s.click(item_x(3), y);
    s.shift_click(item_x(0), y);
    let routes: Vec<Route> = events
        .borrow()
        .iter()
        .filter_map(|e| match e {
            BarEvent::Route(r) => Some(r.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        routes,
        [
            Route::CaptureScrollRegion,
            Route::CaptureScroll,
            Route::CaptureRegion
        ]
    );
}

#[test]
fn a_bar_without_a_recorder_disables_recording_and_says_why() {
    let h = 480.;
    let mut s = Surface::open(BAR.width as f32, h, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), true, cx)
    });
    let view = s.view.clone();
    s.click(RECORDING_X, h - 32.);
    assert_eq!(s.app.cx.update(|cx| view.read(cx).popup()), None);
    let hint = s.app.cx.update(|cx| view.read(cx).recording_hint());
    assert!(hint.contains("No screen recorder is installed"), "{hint}");
}

mod control_bar;
mod menu;
mod placement;

fn placed(id: &str, workspace: Option<&str>, shown: bool) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        workspace: workspace.map(str::to_owned),
        logical: shown.then(|| Rect::new(0., 0., 10., 10.)),
        ..window(id)
    }
}

#[test]
fn windows_group_by_workspace_with_the_ones_on_screen_first() {
    use es_overlays::record::window_groups;
    let windows = [
        placed("a", Some("10"), false),
        placed("b", Some("2"), false),
        placed("c", Some("3"), true),
        placed("d", Some("10"), false),
        placed("e", None, false),
        placed("f", Some("mail"), false),
    ];
    let groups = window_groups(&windows);
    let names: Vec<_> = groups.iter().map(|g| g.workspace.as_deref()).collect();
    assert_eq!(
        names,
        [None, Some("3"), Some("2"), Some("10"), Some("mail")]
    );
    assert!(groups[1].shown && !groups[2].shown);
    assert_eq!(groups[3].windows, [0, 3], "list order inside a group");
}

#[test]
fn the_window_menu_has_a_header_per_workspace_and_picks_the_right_window() {
    let mut s = Surface::open(WIDTH as f32, 480., |_| {
        RecordingOptions::new(vec![output("A", 0., 1280., 720.)], devices())
    });
    let (events, _sub) = s.record::<OptionsEvent>();
    let view = s.view.clone();
    let windows = vec![
        placed("far", Some("2"), false),
        placed("near", Some("1"), true),
    ];
    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.set_windows(windows, cx)));
    s.click(WINDOW_X, ROW_Y);
    // Rows: header 1, near, header 2, far, refresh.
    s.click(LESS_X, menu_row(3));
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::Start(r)) if matches!(&r.target, RecordingTarget::Window(w) if w.id.0 == "far")
    ));
}
