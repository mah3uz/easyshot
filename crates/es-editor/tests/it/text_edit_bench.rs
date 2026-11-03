#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::text_edit_support;

use canvas_support::*;
use es_model::TextAlign;
use gpui::EntityInputHandler;
use std::time::Instant;
use text_edit_support::*;

const KEYSTROKE_BUDGET_MS: f64 = 8.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn a_keystroke_in_a_200_character_text_draws_within_budget() {
    let line = "The quick brown fox jumps over the lazy dog, fifty";
    let body = [line; 4].join("\n");
    assert_eq!(body.chars().count(), 203);
    let shape = text_shape(300.0, 400.0, &body, TextAlign::Middle, 0.2);
    let id = shape.id;
    let (mut app, session) = open(
        1400.0,
        900.0,
        opened(2560, 1600, vec![shape], Default::default()),
    );
    begin(&mut app, &session, id);
    let canvas = canvas(&app);
    let handle = app.window.into();
    let mut times = Vec::new();
    for i in 0..120 {
        let ch = if i % 10 == 9 { " " } else { "x" };
        let took = app
            .cx
            .update_window(handle, |_, window, cx| {
                let t = Instant::now();
                canvas.update(cx, |c, cx| c.replace_text_in_range(None, ch, window, cx));
                window.draw(cx).clear(cx);
                t.elapsed()
            })
            .expect("window is open");
        app.settle();
        times.push(took.as_secs_f64() * 1000.0);
    }
    assert_eq!(
        text_of(&app, &session, id).map(|t| t.chars().count()),
        Some(323),
        "every keystroke reached the text"
    );
    times.sort_by(f64::total_cmp);
    let (p50, p95) = (percentile(&times, 0.5), percentile(&times, 0.95));
    eprintln!("frame CPU per keystroke in a 4-line text: p50 {p50:.2} ms, p95 {p95:.2} ms");
    assert!(p95 <= KEYSTROKE_BUDGET_MS, "p95 {p95:.2} ms");
}
