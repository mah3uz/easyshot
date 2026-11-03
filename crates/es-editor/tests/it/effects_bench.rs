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
