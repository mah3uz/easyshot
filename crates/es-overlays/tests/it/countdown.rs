#![allow(clippy::unwrap_used)]

use crate::window_mode_support;

use es_app::capture_coordinator::Timer;
use es_overlays::countdown::{
    Clock, Countdown, CountdownStyle, MIN_TIMED_SECONDS, exit_duration, timer_seconds,
};
use es_ui::motion::{COUNTDOWN_IN, COUNTDOWN_IN_REDUCED, DIGIT_ROLL, set_reduce_motion};
use es_ui::tokens::ThemeMode;
use gpui::Task;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};
use window_mode_support::{Surface, bluish, rgb};

const W: f32 = 400.;
const H: f32 = 300.;

struct Fake {
    clock: Clock,
    now: Rc<Cell<Instant>>,
    asked: Rc<RefCell<Vec<Duration>>>,
}

/// Each sleep returns at once and moves time on by what was asked.
fn fake() -> Fake {
    let now = Rc::new(Cell::new(Instant::now()));
    let asked = Rc::new(RefCell::new(Vec::new()));
    let (n, a) = (now.clone(), asked.clone());
    let clock = Clock {
        now: Rc::new({
            let now = now.clone();
            move || now.get()
        }),
        sleep: Rc::new(move |d, _| {
            a.borrow_mut().push(d);
            n.set(n.get() + d);
            Task::ready(())
        }),
    };
    Fake { clock, now, asked }
}

fn counts_down(reduce: bool) -> Vec<Duration> {
    let f = fake();
    let clock = f.clock.clone();
    let mut s = Surface::open(W, H, |_| Countdown::new(CountdownStyle::Capture, 3, clock));
    s.app
        .cx
        .update(|cx| es_ui::motion::set_reduce_motion(reduce, cx));
    let view = s.view.clone();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let watch = seen.clone();
    let _sub = s.app.cx.update(|cx| {
        cx.observe(&view, move |v, cx| {
            let v = v.read(cx);
            let state = (v.remaining(), v.exiting(), v.finished());
            let mut seen = watch.borrow_mut();
            if seen.last() != Some(&state) {
                seen.push(state);
            }
        })
    });
    let task = s.app.cx.update(|cx| Countdown::run(&view, cx));
    s.app
        .until("the countdown finishes", |cx| view.read(cx).finished());
    drop(task);
    assert_eq!(
        *seen.borrow(),
        vec![
            (3, false, false),
            (2, false, false),
            (1, false, false),
            (1, true, false),
            (1, true, true)
        ],
        "3, 2, 1, then the exit, then done"
    );
    let _ = f.now;
    f.asked.take()
}

#[test]
fn ticks_once_a_second_then_exits_with_a_spring() {
    let secs = Duration::from_secs(1);
    assert_eq!(
        counts_down(false),
        vec![secs, secs, secs, Duration::from_millis(300)]
    );
}

#[test]
fn reduce_motion_shortens_the_exit_to_a_quick_fade() {
    assert_eq!(exit_duration(true), Duration::from_millis(120));
    let secs = Duration::from_secs(1);
    assert_eq!(
        counts_down(true),
        vec![secs, secs, secs, Duration::from_millis(120)]
    );
}

#[test]
fn the_ring_sweeps_away_from_the_top_clockwise() {
    let f = fake();
    let clock = f.clock.clone();
    let mut s = Surface::open(W, H, |_| Countdown::new(CountdownStyle::Capture, 3, clock));
    f.now.set(f.now.get() + Duration::from_millis(1400));
    s.app.cx.update(|cx| s.view.update(cx, |_, cx| cx.notify()));
    let shot = s.app.screenshot("countdown-mid-sweep");
    let (cx, cy, r) = (W / 2., H / 2., 152. / 2. - 5. - 2.5);
    assert!(bluish(rgb(&shot, cx + r, cy)), "the right is still swept");
    assert!(bluish(rgb(&shot, cx, cy + r)), "the bottom is still swept");
    assert!(!bluish(rgb(&shot, cx - r, cy)), "the left has run out");
    assert!(
        !bluish(rgb(&shot, cx - 3., cy - r)),
        "just left of the top has run out"
    );
}

#[test]
fn a_timer_with_seconds_counts_them_and_the_setting_counts_at_least_three() {
    assert_eq!(timer_seconds(Timer::Seconds(10), 0), 10);
    assert_eq!(timer_seconds(Timer::Seconds(1), 5), 1);
    assert_eq!(timer_seconds(Timer::Setting, 0), MIN_TIMED_SECONDS);
    assert_eq!(timer_seconds(Timer::Setting, 5), 5);
}

const DIAL: f32 = 152.;

struct Timed {
    s: Surface<Countdown>,
    now: Rc<Cell<Instant>>,
}

/// Time moves only when the test moves it; a sleep waits on the test executor's clock.
fn timed(mode: ThemeMode, reduce: bool, style: CountdownStyle) -> Timed {
    let now = Rc::new(Cell::new(Instant::now()));
    let clock = Clock {
        now: Rc::new({
            let now = now.clone();
            move || now.get()
        }),
        sleep: Rc::new(|d, cx| cx.background_executor().timer(d)),
    };
    let mut s = Surface::open(W, H, |_| Countdown::new(style, 3, clock));
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(mode, cx);
        set_reduce_motion(reduce, cx);
    });
    Timed { s, now }
}

impl Timed {
    fn at(&mut self, by: Duration) {
        self.now.set(self.now.get() + by);
    }

    fn frame(&mut self, name: &str) -> image::RgbaImage {
        let handle = self.s.app.window.into();
        self.s
            .app
            .cx
            .update_window(handle, |_, window, _| window.refresh())
            .unwrap();
        let image = self.s.app.screenshot(name);
        if let Some(dir) = std::env::var_os("EASYSHOT_PICTURES") {
            std::fs::create_dir_all(&dir).unwrap();
            image
                .save(std::path::Path::new(&dir).join(format!("{name}.png")))
                .unwrap();
        }
        image
    }
}

/// How wide the dial is drawn across its middle, against the scrim at the corner.
fn dial_width(shot: &image::RgbaImage) -> f32 {
    let scrim = rgb(shot, 1., 1.);
    let differs = |p: [u8; 3]| (0..3).any(|i| p[i].abs_diff(scrim[i]) > 8);
    let xs: Vec<f32> = (0..W as u32)
        .map(|x| x as f32)
        .filter(|&x| differs(rgb(shot, x, H / 2.)))
        .collect();
    match (xs.first(), xs.last()) {
        (Some(a), Some(b)) => b - a + 1.,
        _ => 0.,
    }
}

/// How tall the digits' ink stands inside the ring, against the dial's face above them.
fn ink_height(shot: &image::RgbaImage) -> f32 {
    let (cx, cy) = (W / 2., H / 2.);
    let face = rgb(shot, cx, cy - 48.);
    let ink = |p: [u8; 3]| (0..3).any(|i| p[i].abs_diff(face[i]) > 30);
    let ys: Vec<f32> = (-48..=48)
        .map(|dy| cy + dy as f32)
        .filter(|&y| (-24..=24).any(|dx| ink(rgb(shot, cx + dx as f32, y))))
        .collect();
    match (ys.first(), ys.last()) {
        (Some(a), Some(b)) => b - a + 1.,
        _ => 0.,
    }
}

fn entry(mode: ThemeMode, tag: &str) {
    let mut t = timed(mode, false, CountdownStyle::Capture);
    let start = t.frame(&format!("countdown-entry-{tag}-0-start"));
    assert_eq!(dial_width(&start), 0., "the dial starts unseen");
    t.at(COUNTDOWN_IN.duration / 10);
    let mid = t.frame(&format!("countdown-entry-{tag}-1-mid"));
    let early = dial_width(&mid);
    assert!(
        early > DIAL * 0.82 - 2. && early < DIAL - 6.,
        "early on it is still growing: {early}"
    );
    t.at(COUNTDOWN_IN.duration);
    let end = t.frame(&format!("countdown-entry-{tag}-2-end"));
    assert!(
        (dial_width(&end) - DIAL).abs() < 3.,
        "it arrives at full size: {}",
        dial_width(&end)
    );
}

#[test]
fn the_dial_springs_in_from_smaller_and_unseen_dark() {
    entry(ThemeMode::Dark, "dark");
}

#[test]
fn the_dial_springs_in_from_smaller_and_unseen_light() {
    entry(ThemeMode::Light, "light");
}

#[test]
fn with_reduce_motion_the_dial_only_fades_in() {
    let mut t = timed(ThemeMode::Light, true, CountdownStyle::Capture);
    t.at(COUNTDOWN_IN_REDUCED.duration / 2);
    let mid = t.frame("countdown-entry-reduced-mid");
    assert!(
        (dial_width(&mid) - DIAL).abs() < 3.,
        "full size while it fades: {}",
        dial_width(&mid)
    );
    t.at(COUNTDOWN_IN_REDUCED.duration);
    assert!(
        t.frame("countdown-entry-reduced-end") != mid,
        "and it fades"
    );
}

const SETTLED: Duration = Duration::from_millis(720);

fn running(t: &mut Timed) -> Task<()> {
    let view = t.s.view.clone();
    let task = t.s.app.cx.update(|cx| Countdown::run(&view, cx));
    t.s.app.settle();
    t.at(SETTLED);
    task
}

fn tick(t: &mut Timed) {
    t.at(Duration::from_secs(1) - SETTLED);
    t.s.app.cx.advance_clock(Duration::from_secs(1));
    t.s.app.settle();
}

fn roll(mode: ThemeMode, tag: &str) {
    let mut t = timed(mode, false, CountdownStyle::Capture);
    let _task = running(&mut t);
    let three = t.frame(&format!("countdown-roll-{tag}-0-three"));
    let rest = ink_height(&three);
    tick(&mut t);
    assert_eq!(t.s.app.cx.update(|cx| t.s.view.read(cx).remaining()), 2);
    let start = t.frame(&format!("countdown-roll-{tag}-1-start"));
    assert!(
        (ink_height(&start) - rest).abs() < 2.,
        "the 3 is still in place as the 2 arrives"
    );
    t.at(DIGIT_ROLL.duration / 8);
    let mid = t.frame(&format!("countdown-roll-{tag}-2-mid"));
    assert!(
        ink_height(&mid) > rest + 10.,
        "mid-roll the 3 drops while the 2 comes down: {} vs {rest}",
        ink_height(&mid)
    );
    t.at(DIGIT_ROLL.duration);
    let end = t.frame(&format!("countdown-roll-{tag}-3-end"));
    assert!(
        (ink_height(&end) - rest).abs() < 6.,
        "the 2 settles where the 3 stood"
    );
}

#[test]
fn each_digit_rolls_down_into_place_dark() {
    roll(ThemeMode::Dark, "dark");
}

#[test]
fn each_digit_rolls_down_into_place_light() {
    roll(ThemeMode::Light, "light");
}

#[test]
fn with_reduce_motion_the_digit_swaps() {
    let mut t = timed(ThemeMode::Light, true, CountdownStyle::Capture);
    let _task = running(&mut t);
    tick(&mut t);
    let now = t.frame("countdown-roll-reduced-start");
    t.at(DIGIT_ROLL.duration / 4);
    assert!(
        t.frame("countdown-roll-reduced-later") == now,
        "the 2 is in place at once"
    );
}

#[test]
fn recording_badge_pictures() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let mut t = timed(mode, false, CountdownStyle::Recording);
        let _task = running(&mut t);
        tick(&mut t);
        for (i, at) in [Duration::ZERO, DIGIT_ROLL.duration / 8, DIGIT_ROLL.duration]
            .into_iter()
            .enumerate()
        {
            t.at(at);
            t.frame(&format!("countdown-badge-roll-{tag}-{i}"));
        }
    }
}
