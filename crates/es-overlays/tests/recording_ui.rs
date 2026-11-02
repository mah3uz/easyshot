mod window_mode_support;

use es_geom::{Point, Rect, Size};
use es_model::platform::{OutputId, OutputInfo, Transform, WindowId, WindowInfo};
use es_overlays::record::bar::{BAR, Popup};
use es_overlays::record::control::{BAR as CONTROL, CONFIRM};
use es_overlays::record::options::WIDTH;
use es_overlays::record::{
    BarEvent, CaptureBar, Confirm, ControlBar, ControlEvent, OptionsEvent, Phase, Placement,
    Recorded, RecordingOptions, RecordingTarget, control_bar, highlight_strips,
};
use es_protocol::Route;
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
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), cx)
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
fn timer_and_recording_open_their_popups_above_the_bar() {
    let h = 480.;
    let mut s = Surface::open(BAR.width as f32, h, |cx| {
        CaptureBar::new(vec![output("A", 0., 1280., 720.)], devices(), cx)
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

const PAUSE_X: f32 = 1. + 132. + 1. + 21.;
const RESTART_X: f32 = PAUSE_X + 43.;
const DISCARD_X: f32 = RESTART_X + 43.;
const STOP_X: f32 = 60.;

#[test]
fn the_timer_leaves_out_paused_time() {
    let clock = Arc::new(TestClock::default());
    let c = clock.clone();
    let mut s = Surface::open(CONTROL.width as f32, CONTROL.height as f32, move |cx| {
        ControlBar::new(c, cx)
    });
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
    s.app.cx.update(|cx| view.update(cx, |_, cx| cx.notify()));
    let shot = s.app.screenshot("control-bar-recording");
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(ThemeMode::Light, cx);
        view.update(cx, |_, cx| cx.notify());
    });
    s.app.screenshot("control-bar-recording-light");
    let red = rgb(&shot, 18., 19.);
    assert!(
        i32::from(red[0]) > i32::from(red[2]) + 60,
        "the stop glyph is red: {red:?}"
    );
    let tray = s.app.cx.update(|cx| view.read(cx).tray_menu());
    assert_eq!(tray.elapsed, "00:07");
    assert_eq!(tray.items[0].route, Route::RecordStop);
    assert!(tray.items.iter().all(|i| i.enabled));
}

#[test]
fn nothing_answers_while_starting_or_finishing() {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(CONTROL.width as f32, CONTROL.height as f32, move |cx| {
        ControlBar::new(clock, cx)
    });
    let (events, _sub) = s.record::<ControlEvent>();
    for x in [STOP_X, PAUSE_X, RESTART_X, DISCARD_X] {
        s.click(x, 19.);
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
    let tray = s.app.cx.update(|cx| view.read(cx).tray_menu());
    assert!(tray.items.iter().all(|i| !i.enabled));
}

// The confirm card's buttons are 88 wide, 8 apart, inside 14 of padding.
const CANCEL: (f32, f32) = (185., 96.);
const OK: (f32, f32) = (281., 96.);

#[test]
fn start_over_and_discard_ask_first() {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(CONFIRM.width as f32, CONFIRM.height as f32, move |cx| {
        ControlBar::new(clock, cx)
    });
    let view = s.view.clone();
    s.app.cx.update(|cx| view.update(cx, |b, cx| b.started(cx)));
    let (events, _sub) = s.record::<ControlEvent>();
    let confirm = |s: &mut Surface<ControlBar>| {
        let v = view.clone();
        s.app.cx.update(|cx| v.read(cx).confirm())
    };
    s.click(STOP_X, 19.);
    s.click(PAUSE_X, 19.);
    assert_eq!(
        *events.borrow(),
        vec![
            ControlEvent::Route(Route::RecordStop),
            ControlEvent::Route(Route::RecordPause)
        ]
    );
    events.borrow_mut().clear();

    s.click(RESTART_X, 19.);
    assert_eq!(confirm(&mut s), Some(Confirm::Restart));
    s.app.screenshot("control-bar-confirm-restart");
    s.click(CANCEL.0, CANCEL.1);
    assert_eq!(confirm(&mut s), None);
    assert_eq!(
        *events.borrow(),
        vec![ControlEvent::Resized, ControlEvent::Resized]
    );
    s.click(RESTART_X, 19.);
    s.click(OK.0, OK.1);
    assert_eq!(
        events.borrow().last(),
        Some(&ControlEvent::Route(Route::RecordRestart))
    );

    s.click(DISCARD_X, 19.);
    assert_eq!(confirm(&mut s), Some(Confirm::Discard));
    let size = s.app.cx.update(|cx| view.read(cx).size());
    assert_eq!(size, CONFIRM);
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
    assert_eq!(routes, 2, "only the confirm buttons act");
}

const BARSIZE: Size = CONTROL;

fn on(placement: &Placement) -> (String, Rect) {
    match placement {
        Placement::On { output, origin } => (
            output.0.clone(),
            Rect::new(
                origin.x,
                origin.y,
                origin.x + BARSIZE.width,
                origin.y + BARSIZE.height,
            ),
        ),
        Placement::Tray => panic!("expected a bar on screen"),
    }
}

fn clear_of(bar: Rect, area: Rect) -> bool {
    let i = bar.intersect(area);
    i.width() <= 0. || i.height() <= 0.
}

#[test]
fn the_control_bar_stays_out_of_an_area_near_each_edge() {
    let one = vec![output("A", 0., 1280., 720.)];
    let cases = [
        ("top", Rect::new(100., 0., 1000., 200.)),
        ("bottom", Rect::new(100., 400., 1000., 720.)),
        ("left", Rect::new(0., 100., 500., 600.)),
        ("right", Rect::new(900., 100., 1280., 700.)),
        ("middle", Rect::new(300., 100., 900., 600.)),
    ];
    for (name, area) in cases {
        let placed = control_bar(&Recorded::Area(area), &one, BARSIZE);
        let (out, bar) = on(&placed);
        assert_eq!(out, "A", "{name}");
        assert!(clear_of(bar, area), "{name}: {bar:?} covers {area:?}");
        assert!(
            bar.y0 >= 4. && bar.y1 <= 716.,
            "{name}: {bar:?} leaves the output"
        );
    }
    let low = Rect::new(100., 400., 1000., 720.);
    let (_, bar) = on(&control_bar(&Recorded::Area(low), &one, BARSIZE));
    assert_eq!(bar.y1, 392., "no room below, so just above");
    let mid = Rect::new(300., 100., 900., 650.);
    let (_, bar) = on(&control_bar(&Recorded::Area(mid), &one, BARSIZE));
    assert_eq!(bar.y0, 658., "home would cover it, so just below");
    let small = Rect::new(100., 0., 1000., 200.);
    let (_, bar) = on(&control_bar(&Recorded::Area(small), &one, BARSIZE));
    assert_eq!(
        bar.origin(),
        Point::new(508., 634.),
        "home, as the capture bar"
    );
}

#[test]
fn an_area_with_no_room_around_it_sends_the_bar_elsewhere() {
    let tall = Rect::new(0., 30., 700., 700.);
    let one = vec![output("A", 0., 1280., 720.)];
    assert_eq!(
        control_bar(&Recorded::Area(tall), &one, BARSIZE),
        Placement::Tray
    );
    let two = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let (out, bar) = on(&control_bar(&Recorded::Area(tall), &two, BARSIZE));
    assert_eq!(out, "B");
    assert!(bar.x0 >= 1280.);
}

#[test]
fn a_full_display_recording_uses_another_output_or_the_tray() {
    let one = vec![output("A", 0., 1280., 720.)];
    let whole = Recorded::Display(OutputId("A".into()));
    assert_eq!(control_bar(&whole, &one, BARSIZE), Placement::Tray);
    let two = vec![
        output("A", 0., 1280., 720.),
        output("B", 1280., 1920., 1080.),
    ];
    let (out, bar) = on(&control_bar(&whole, &two, BARSIZE));
    assert_eq!(out, "B");
    assert_eq!(
        bar.origin(),
        Point::new(1280. + 960. - 132., 1080. - 48. - 38.)
    );
    let (out, _) = on(&control_bar(
        &Recorded::Display(OutputId("B".into())),
        &two,
        BARSIZE,
    ));
    assert_eq!(out, "A");
}

#[test]
fn the_highlight_dims_everything_but_the_area() {
    let out = Rect::new(0., 0., 1280., 720.);
    let area = Rect::new(100., 50., 600., 400.);
    let strips = highlight_strips(area, out);
    assert_eq!(strips.len(), 4);
    let covered: f64 = strips.iter().map(|r| r.width() * r.height()).sum();
    assert_eq!(
        covered,
        out.width() * out.height() - area.width() * area.height()
    );
    assert!(strips.iter().all(|r| clear_of(*r, area)));
    assert_eq!(
        highlight_strips(Rect::new(0., 0., 600., 720.), out).len(),
        1
    );
}
