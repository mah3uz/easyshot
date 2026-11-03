use crate::window_mode_support;

use es_overlays::countdown::{Clock, Countdown, CountdownStyle, exit_duration};
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
