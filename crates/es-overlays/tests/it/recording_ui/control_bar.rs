use super::*;
use es_record::AudioSource;
use es_ui::motion::{HANDOFF_IN, set_reduce_motion};
use std::cell::Cell;
use std::rc::Rc;

const MUTE_WIDTH: f32 = 43.;
const FIRST_MUTE_X: f32 = PAUSE_X;

fn open(audio: Vec<AudioSource>) -> Surface<ControlBar> {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let width = CONTROL.width as f32 + MUTE_WIDTH * audio.len() as f32;
    let mut s = Surface::open(width, (CONTROL.height + TIP_ROOM) as f32, move |cx| {
        ControlBar::new(clock, audio, cx)
    });
    let view = s.view.clone();
    s.app.cx.update(|cx| view.update(cx, |b, cx| b.started(cx)));
    s.app.cx.advance_clock(Duration::from_millis(300));
    s
}

fn size(s: &mut Surface<ControlBar>) -> Size {
    let view = s.view.clone();
    s.app.cx.update(|cx| view.read(cx).size())
}

#[test]
fn a_mute_button_exists_only_for_each_source_the_recording_takes() {
    let mut none = open(Vec::new());
    assert_eq!(size(&mut none).width, CONTROL.width);
    let (events, _sub) = none.record::<ControlEvent>();
    none.click(FIRST_MUTE_X, BAR_Y);
    assert_eq!(
        *events.borrow(),
        [ControlEvent::Route(Route::RecordPause)],
        "with no sound the first button after Stop is Pause"
    );

    let mut mic = open(vec![AudioSource::Mic]);
    assert_eq!(size(&mut mic).width, CONTROL.width + f64::from(MUTE_WIDTH));
    let (events, _sub) = mic.record::<ControlEvent>();
    mic.click(FIRST_MUTE_X, BAR_Y);
    mic.click(FIRST_MUTE_X + MUTE_WIDTH, BAR_Y);
    assert_eq!(
        *events.borrow(),
        [
            ControlEvent::Mute(AudioSource::Mic, true),
            ControlEvent::Route(Route::RecordPause)
        ]
    );

    let mut both = open(vec![AudioSource::System, AudioSource::Mic]);
    assert_eq!(
        size(&mut both).width,
        CONTROL.width + 2. * f64::from(MUTE_WIDTH)
    );
    let (events, _sub) = both.record::<ControlEvent>();
    both.click(FIRST_MUTE_X, BAR_Y);
    both.click(FIRST_MUTE_X + MUTE_WIDTH, BAR_Y);
    assert_eq!(
        *events.borrow(),
        [
            ControlEvent::Mute(AudioSource::System, true),
            ControlEvent::Mute(AudioSource::Mic, true)
        ]
    );
}

#[test]
fn a_button_asks_to_unmute_only_after_the_recorder_confirmed_the_mute() {
    let mut s = open(vec![AudioSource::Mic]);
    let (events, _sub) = s.record::<ControlEvent>();
    let view = s.view.clone();
    let muted = |s: &mut Surface<ControlBar>| {
        let v = view.clone();
        s.app.cx.update(|cx| v.read(cx).is_muted(AudioSource::Mic))
    };
    s.click(FIRST_MUTE_X, BAR_Y);
    s.click(FIRST_MUTE_X, BAR_Y);
    assert_eq!(
        *events.borrow(),
        [
            ControlEvent::Mute(AudioSource::Mic, true),
            ControlEvent::Mute(AudioSource::Mic, true)
        ],
        "a click that the recorder did not confirm leaves the bar unmuted"
    );
    assert!(!muted(&mut s));
    s.app.cx.update(|cx| {
        view.update(cx, |b, cx| b.set_muted(AudioSource::Mic, true, cx));
    });
    assert!(muted(&mut s));
    s.app.screenshot("control-bar-muted-mic");
    events.borrow_mut().clear();
    s.click(FIRST_MUTE_X, BAR_Y);
    assert_eq!(
        *events.borrow(),
        [ControlEvent::Mute(AudioSource::Mic, false)]
    );
}

struct Drag {
    s: Surface<ControlBar>,
    pointer: Rc<Cell<Option<Point>>>,
}

const OUTPUT: Rect = Rect::new(0., 0., 1280., 720.);
const START: Point = Point::new(500., 600.);

fn drag_rig() -> Drag {
    let mut s = open(Vec::new());
    let pointer = Rc::new(Cell::new(None));
    let global = pointer.clone();
    let view = s.view.clone();
    s.app.cx.update(|cx| {
        view.update(cx, |b, _| {
            b.place(START, OUTPUT, Rc::new(move || global.get()))
        });
    });
    Drag { s, pointer }
}

impl Drag {
    const GRIP: (f32, f32) = (9., BAR_Y);

    fn press(&mut self, at: Point) {
        self.pointer.set(Some(at));
        self.s.down(Self::GRIP.0, Self::GRIP.1);
    }

    fn move_to(&mut self, at: Point) {
        self.pointer.set(Some(at));
        self.s.hover(Self::GRIP.0, Self::GRIP.1, true);
    }

    fn origin(&mut self) -> Point {
        let view = self.s.view.clone();
        self.s.app.cx.update(|cx| view.read(cx).origin())
    }
}

#[test]
fn dragging_the_grip_moves_the_bar_and_never_off_its_output() {
    let mut d = drag_rig();
    let (events, _sub) = d.s.record::<ControlEvent>();
    let grab = Point::new(509., 649.);
    d.press(grab);
    d.move_to(Point::new(grab.x + 10., grab.y - 20.));
    assert_eq!(d.origin(), Point::new(510., 580.));
    d.move_to(Point::new(grab.x + 5000., grab.y + 5000.));
    let bar = size(&mut d.s);
    assert_eq!(
        d.origin(),
        Point::new(OUTPUT.x1 - bar.width, OUTPUT.y1 - bar.height),
        "stops at the bottom-right corner"
    );
    d.move_to(Point::new(grab.x - 5000., grab.y - 5000.));
    assert_eq!(
        d.origin(),
        Point::new(0., 0.),
        "stops at the top-left corner"
    );
    d.s.up(Drag::GRIP.0, Drag::GRIP.1);
    assert_eq!(events.borrow().last(), Some(&ControlEvent::Dropped));
    assert!(events.borrow().contains(&ControlEvent::Moved));
}

#[test]
fn only_the_grip_drags() {
    let mut d = drag_rig();
    let (events, _sub) = d.s.record::<ControlEvent>();
    d.pointer.set(Some(Point::new(600., 649.)));
    d.s.down(STOP_X, BAR_Y);
    d.pointer.set(Some(Point::new(700., 500.)));
    d.s.hover(STOP_X, BAR_Y, true);
    d.s.up(STOP_X, BAR_Y);
    assert_eq!(d.origin(), START);
    assert!(
        !events
            .borrow()
            .iter()
            .any(|e| matches!(e, ControlEvent::Moved | ControlEvent::Dropped))
    );
}

fn entering(reduce: bool) -> (bool, bool) {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(
        CONTROL.width as f32,
        (CONTROL.height + TIP_ROOM) as f32,
        move |cx| {
            set_reduce_motion(reduce, cx);
            ControlBar::new(clock, Vec::new(), cx)
        },
    );
    let view = s.view.clone();
    let at_start = s.app.cx.update(|cx| view.read(cx).entering(cx));
    s.app.cx.advance_clock(HANDOFF_IN.duration * 5 / 6);
    let mid = s.app.cx.update(|cx| view.read(cx).entering(cx));
    assert_eq!(
        mid, !reduce,
        "a spring runs its whole time, past its first arrival"
    );
    s.app.cx.advance_clock(HANDOFF_IN.duration / 6);
    let later = s.app.cx.update(|cx| view.read(cx).entering(cx));
    (at_start, later)
}

#[test]
fn the_hand_off_from_the_capture_bar_plays_unless_reduce_motion_is_on() {
    assert_eq!(
        entering(false),
        (true, false),
        "fades and grows in, then done"
    );
    assert_eq!(
        entering(true),
        (false, false),
        "Reduce Motion shows it at once"
    );
}

fn has_red(shot: &image::RgbaImage, centre: f32) -> bool {
    (-8..=8).any(|dx| {
        (-8..=8).any(|dy| {
            let p = rgb(shot, centre + dx as f32, BAR_Y + dy as f32);
            i32::from(p[0]) > i32::from(p[2]) + 60
        })
    })
}

#[test]
fn a_muted_source_is_drawn_in_red_in_both_themes() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut s = open(vec![AudioSource::System, AudioSource::Mic]);
        let view = s.view.clone();
        s.app.cx.update(|cx| {
            es_ui::theme::set_mode(mode, cx);
            view.update(cx, |_, cx| cx.notify());
        });
        let open_shot = s.app.screenshot(&format!("control-bar-two-sources-{tag}"));
        assert!(!has_red(&open_shot, FIRST_MUTE_X + MUTE_WIDTH), "{tag}");
        s.app.cx.update(|cx| {
            view.update(cx, |b, cx| b.set_muted(AudioSource::Mic, true, cx));
        });
        let mic = s.app.screenshot(&format!("control-bar-mic-muted-{tag}"));
        assert!(has_red(&mic, FIRST_MUTE_X + MUTE_WIDTH), "{tag}");
        assert!(!has_red(&mic, FIRST_MUTE_X), "{tag}");
        s.app.cx.update(|cx| {
            view.update(cx, |b, cx| b.set_muted(AudioSource::System, true, cx));
        });
        let both = s.app.screenshot(&format!("control-bar-both-muted-{tag}"));
        assert!(has_red(&both, FIRST_MUTE_X), "{tag}");
    }
}

fn arriving(name: &str, frames: usize, step: Duration) -> Vec<image::RgbaImage> {
    let clock: Arc<dyn Clock> = Arc::new(TestClock::default());
    let mut s = Surface::open(
        CONTROL.width as f32,
        (CONTROL.height + TIP_ROOM) as f32,
        move |cx| ControlBar::new(clock, Vec::new(), cx),
    );
    let handle = s.app.window.into();
    (0..frames)
        .map(|i| {
            s.app
                .cx
                .update_window(handle, |_, window, _| window.refresh())
                .unwrap_or_default();
            let shot = s.app.screenshot(&format!("{name}-{i}"));
            s.app.cx.advance_clock(step);
            shot
        })
        .collect()
}

/// The drawn width of the bar along its middle row, and its strongest contrast there with
/// `back`, the surface before the bar is drawn.
fn drawn(shot: &image::RgbaImage, back: &image::RgbaImage) -> (usize, u32) {
    let scale = shot.width() as f32 / CONTROL.width as f32;
    let y = (BAR_Y * scale) as u32;
    let behind = back.get_pixel(0, y).0;
    let diffs: Vec<u32> = (0..shot.width())
        .map(|x| {
            let p = shot.get_pixel(x, y).0;
            (0..3).map(|c| u32::from(p[c].abs_diff(behind[c]))).sum()
        })
        .collect();
    (
        diffs.iter().filter(|d| **d > 12).count(),
        diffs.iter().copied().max().unwrap_or(0),
    )
}

#[test]
fn the_control_bar_springs_in_growing_and_fading_from_the_capture_bar_it_replaces() {
    let frames = arriving("control-bar-enter", 10, HANDOFF_IN.duration / 8);
    let rest = drawn(&frames[9], &frames[0]);
    let early = drawn(&frames[1], &frames[0]);
    assert!(
        early.0 + 6 < rest.0,
        "it starts narrower than it rests: {early:?} then {rest:?}"
    );
    assert!(early.1 < rest.1, "and faint: {early:?} then {rest:?}");
    assert_eq!(
        drawn(&frames[8], &frames[0]),
        rest,
        "and is at rest once its spring is done"
    );
}
