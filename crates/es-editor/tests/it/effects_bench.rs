#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::*;
use es_editor::canvas::effects::{SETTLE_DELAY, Showing};
use es_model::{BackgroundColor, BackgroundSettings, BackgroundStyle};
use std::time::Instant;

const LIVE_BUDGET_MS: f64 = 16.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn stage() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Solid(BackgroundColor::custom(0.25, 0.45, 0.85)),
        ..BackgroundSettings::default()
    }
}

/// Frame time for each step of a slider drag, from the change to the drawn frame.
fn drag(name: &str, start: &BackgroundSettings, step: impl Fn(&mut BackgroundSettings, usize)) {
    let (mut app, _session) = open(
        1400.0,
        900.0,
        opened(2560, 1600, every_kind(true, true), start.clone()),
    );
    let c = canvas(&app);
    let t = Instant::now();
    app.cx.advance_clock(SETTLE_DELAY);
    app.settle();
    let settled = t.elapsed().as_secs_f64() * 1000.0;
    app.cx
        .update(|cx| c.update(cx, |c, cx| c.begin_live_adjust(cx)));
    app.settle();
    let handle = app.window.into();
    let mut times = Vec::new();
    for i in 1..=80 {
        let mut look = start.clone();
        step(&mut look, i);
        let took = app
            .cx
            .update_window(handle, |_, window, cx| {
                let t = Instant::now();
                c.update(cx, |c, cx| c.set_background(look, cx));
                window.draw(cx).clear(cx);
                t.elapsed()
            })
            .expect("window is open");
        app.settle();
        times.push(took.as_secs_f64() * 1000.0);
    }
    let state = c.read_with(&app.cx, |c, _| c.effects());
    assert_eq!(state.showing, Some(Showing::Live));
    let first = times[0];
    times.sort_by(f64::total_cmp);
    let (p50, p95) = (percentile(&times, 0.5), percentile(&times, 0.95));
    let worst = times.last().copied().unwrap_or_default();
    eprintln!(
        "{name}: live frame p50 {p50:.2} ms, p95 {p95:.2} ms, first {first:.2} ms, worst {worst:.2} ms; settled composite {settled:.1} ms"
    );
    assert!(p95 <= LIVE_BUDGET_MS, "{name}: p95 {p95:.2} ms");
    // The first frame of a drag is the one a synchronous rebuild would stall.
    assert!(first <= LIVE_BUDGET_MS, "{name}: first frame {first:.2} ms");
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn dragging_a_camera_slider_draws_each_live_frame_within_budget() {
    let mut start = stage();
    start.camera.tilt_x_degrees = 1.0;
    drag("camera tilt", &start, |s, i| {
        s.camera.tilt_x_degrees = 1.0 + i as f64 * 0.4;
    });
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn dragging_a_blur_slider_draws_each_live_frame_within_budget() {
    let mut start = stage();
    start.progressive_blur.is_enabled = true;
    drag("blur focus size", &start, |s, i| {
        s.progressive_blur.focus_size = 0.2 + i as f64 * 0.007;
    });
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn dragging_a_camera_slider_with_the_blur_on_draws_each_live_frame_within_budget() {
    let mut start = stage();
    start.camera.tilt_x_degrees = 1.0;
    start.progressive_blur.is_enabled = true;
    drag("camera tilt over blur", &start, |s, i| {
        s.camera.tilt_x_degrees = 1.0 + i as f64 * 0.4;
    });
}

type Slider = (&'static str, fn(&mut BackgroundSettings, f64), f64, f64);

const CAMERA_SLIDERS: [Slider; 9] = [
    ("tilt x", |s, v| s.camera.tilt_x_degrees = v, 2.0, 0.4),
    ("tilt y", |s, v| s.camera.tilt_y_degrees = v, 2.0, 0.4),
    ("roll", |s, v| s.camera.roll_degrees = v, 0.0, 0.5),
    ("fov", |s, v| s.camera.field_of_view_degrees = v, 24.0, 0.5),
    ("zoom", |s, v| s.camera.zoom = v, 1.0, -0.005),
    ("pan x", |s, v| s.camera.pan_x = v, 0.0, 0.002),
    ("pan y", |s, v| s.camera.pan_y = v, 0.0, 0.002),
    ("rotate x", |s, v| s.camera.rotation_x_degrees = v, 0.0, 0.4),
    ("rotate y", |s, v| s.camera.rotation_y_degrees = v, 0.0, 0.4),
];

fn p50_p95(mut times: Vec<f64>) -> (f64, f64) {
    times.sort_by(f64::total_cmp);
    (percentile(&times, 0.5), percentile(&times, 0.95))
}

struct SliderTimes {
    /// UI-thread time from the change to the drawn frame.
    ui: (f64, f64),
    /// Time until the live frame for the change has landed.
    live: (f64, f64),
    exact: (f64, f64),
    pixels: Option<(u32, u32)>,
}

/// Live frame times while dragging, and the time of the exact composite for each settled value.
fn camera_slider(capture: (u32, u32), blur: bool, slider: &Slider) -> SliderTimes {
    let (name, set, from, step) = *slider;
    let mut start = stage();
    start.camera.tilt_y_degrees = -6.0;
    start.progressive_blur.is_enabled = blur;
    set(&mut start, from);
    let (mut app, _session) = open(
        1400.0,
        900.0,
        opened(capture.0, capture.1, every_kind(true, true), start.clone()),
    );
    let c = canvas(&app);
    let handle = app.window.into();
    app.cx.advance_clock(SETTLE_DELAY);
    app.settle();

    let mut exact = Vec::new();
    for i in 1..=6 {
        let mut look = start.clone();
        set(&mut look, from + step * 10.0 * f64::from(i));
        app.cx
            .update(|cx| c.update(cx, |c, cx| c.set_background(look, cx)));
        let t = Instant::now();
        app.cx.advance_clock(SETTLE_DELAY);
        app.settle();
        exact.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(
            c.read_with(&app.cx, |c, _| c.effects()).showing,
            Some(Showing::Settled),
            "{name}"
        );
    }

    app.cx
        .update(|cx| c.update(cx, |c, cx| c.begin_live_adjust(cx)));
    app.settle();
    let (mut ui, mut live) = (Vec::new(), Vec::new());
    for i in 1..=30 {
        let mut look = start.clone();
        set(&mut look, from + step * f64::from(i));
        let took = app
            .cx
            .update_window(handle, |_, window, cx| {
                let t = Instant::now();
                c.update(cx, |c, cx| c.set_background(look, cx));
                window.draw(cx).clear(cx);
                t.elapsed()
            })
            .expect("window is open");
        let t = Instant::now();
        app.settle();
        ui.push(took.as_secs_f64() * 1000.0);
        live.push((took + t.elapsed()).as_secs_f64() * 1000.0);
    }
    let state = c.read_with(&app.cx, |c, _| c.effects());
    assert_eq!(state.showing, Some(Showing::Live), "{name}");
    SliderTimes {
        ui: p50_p95(ui),
        live: p50_p95(live),
        exact: p50_p95(exact),
        pixels: state.pixels,
    }
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn camera_sliders_on_large_captures() {
    for capture in [(3840, 2160), (2560, 1440)] {
        for blur in [false, true] {
            let sliders: &[Slider] = if blur {
                &CAMERA_SLIDERS[..1]
            } else {
                &CAMERA_SLIDERS
            };
            for slider in sliders {
                let t = camera_slider(capture, blur, slider);
                eprintln!(
                    "{}x{} {}{} (composite {:?}): ui p50 {:.2} p95 {:.2} ms; live frame p50 {:.2} p95 {:.2} ms; exact p50 {:.2} p95 {:.2} ms",
                    capture.0,
                    capture.1,
                    slider.0,
                    if blur { " over blur" } else { "" },
                    t.pixels,
                    t.ui.0,
                    t.ui.1,
                    t.live.0,
                    t.live.1,
                    t.exact.0,
                    t.exact.1,
                );
            }
        }
    }
}
