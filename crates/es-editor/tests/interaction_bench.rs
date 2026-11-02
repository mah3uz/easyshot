#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod interaction_support;

use canvas_support::*;
use es_model::{AnnoShape, RedactionKind, RedactionProps, ShapeKind, Tool};
use gpui::{MouseButton, MouseMoveEvent, PlatformInput};
use interaction_support::*;
use std::time::Instant;

const DRAW_BUDGET_MS: f64 = 8.0;
const PATCH_BUDGET_MS: f64 = 16.0;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn moved(to: gpui::Point<gpui::Pixels>) -> PlatformInput {
    PlatformInput::MouseMove(MouseMoveEvent {
        position: to,
        pressed_button: Some(MouseButton::Left),
        ..MouseMoveEvent::default()
    })
}

/// Rows across the page, so 5000 points stay on screen 1.5 page pixels apart.
fn zigzag(i: usize) -> (f64, f64) {
    let per_row = 1200;
    let (row, k) = (i / per_row, i % per_row);
    let x = if row % 2 == 0 { k } else { per_row - k } as f64 * 1.5 + 200.0;
    (
        x,
        200.0 + row as f64 * 260.0 + (k as f64 * 0.04).sin() * 60.0,
    )
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn frame_cpu_while_drawing_a_5000_point_stroke_stays_within_budget() {
    let (mut app, session) = open(
        1400.0,
        900.0,
        opened(2560, 1600, every_kind(false, true), Default::default()),
    );
    set_tool(&mut app, &session, Tool::Freehand);
    let (x, y) = zigzag(0);
    let start = at_page(&app, &session, x, y);
    down(&mut app, start);
    let handle = app.window.into();
    let mut times = Vec::new();
    for i in 1..=5000 {
        let (x, y) = zigzag(i);
        let to = at_page(&app, &session, x, y);
        let took = app
            .cx
            .update_window(handle, |_, window, cx| {
                let t = Instant::now();
                window.dispatch_event(moved(to), cx);
                window.draw(cx).clear(cx);
                t.elapsed()
            })
            .expect("window is open");
        app.settle();
        times.push(took.as_secs_f64() * 1000.0);
    }
    let points = session.doc.read_with(&app.cx, |d, _| {
        d.editor().live_stroke().map(|(_, s)| s.raw().len())
    });
    assert_eq!(points, Some(5001), "every move reached the stroke");
    times.sort_by(f64::total_cmp);
    let (p50, p95) = (percentile(&times, 0.5), percentile(&times, 0.95));
    eprintln!("frame CPU while drawing 5000 points: p50 {p50:.2} ms, p95 {p95:.2} ms");
    assert!(p95 <= DRAW_BUDGET_MS, "p95 {p95:.2} ms");
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn a_dragged_redaction_paints_its_new_patch_within_budget() {
    let shape = AnnoShape::new(
        900.0,
        500.0,
        ShapeKind::Redaction(RedactionProps {
            kind: RedactionKind::Blur,
            w: 420.0,
            h: 260.0,
            density: 0.5,
        }),
    );
    let (mut app, session) = open(
        1400.0,
        900.0,
        opened(2560, 1600, vec![shape], Default::default()),
    );
    let c = canvas(&app);
    app.until("a patch", |cx| c.read(cx).redactions().painted().len() == 1);
    set_tool(&mut app, &session, Tool::Select);
    let id = ids(&app, &session)[0];
    select(&mut app, &session, vec![id]);
    let grab = at_page(&app, &session, 1100.0, 630.0);
    down(&mut app, grab);
    let handle = app.window.into();
    let mut times = Vec::new();
    for step in 1..=60 {
        let last = c.read_with(&app.cx, |c, _| c.redactions().painted().first().copied());
        let dx = if (step / 15) % 2 == 0 { 6.0 } else { -6.0 };
        let to = grab + gpui::point(gpui::px(dx * (step % 15) as f32), gpui::px(0.));
        let t = Instant::now();
        app.cx
            .update_window(handle, |_, window, cx| window.dispatch_event(moved(to), cx))
            .expect("window is open");
        loop {
            app.settle();
            let now = c.read_with(&app.cx, |c, _| c.redactions().painted().first().copied());
            if now > last {
                break;
            }
        }
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    let (p50, p95) = (percentile(&times, 0.5), percentile(&times, 0.95));
    eprintln!("redaction drag, move to first paint of its patch: p50 {p50:.2} ms, p95 {p95:.2} ms");
    assert!(p95 <= PATCH_BUDGET_MS, "p95 {p95:.2} ms");
}
