#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;

use canvas_support::*;
use es_geom::PVec;
use es_model::{
    AnnoShape, ArrowProps, Arrowhead, DrawProps, GeoKind, GeoProps, NumberedProps, ShapeKind,
    TextProps,
};
use std::time::{Duration, Instant};

const BUDGET_MS: f64 = 4.0;

fn shapes(n: usize) -> Vec<AnnoShape> {
    (0..n)
        .map(|i| {
            let (x, y) = (
                (i % 20) as f64 * 120.0 + 20.0,
                (i / 20) as f64 * 100.0 + 20.0,
            );
            let colour = swatch(0.2 + (i % 5) as f64 * 0.15, 0.3, 0.8 - (i % 3) as f64 * 0.2);
            let kind = match i % 6 {
                0 => ShapeKind::Geo(GeoProps {
                    w: 90.0,
                    h: 60.0,
                    swatch: colour,
                    stroke_width: 4.0,
                    ..GeoProps::default()
                }),
                1 => ShapeKind::Geo(GeoProps {
                    geo: GeoKind::Ellipse,
                    w: 90.0,
                    h: 60.0,
                    swatch: colour,
                    stroke_width: 4.0,
                    ..GeoProps::default()
                }),
                2 => ShapeKind::Arrow(ArrowProps {
                    start: PVec::new(0.0, 60.0),
                    end: PVec::new(90.0, 0.0),
                    bend: 20.0,
                    arrowhead_end: Arrowhead::Arrow,
                    swatch: colour,
                    stroke_width: 4.0,
                    ..ArrowProps::default()
                }),
                3 => ShapeKind::Draw(DrawProps {
                    points: wave(60, 0.0, 30.0, 90.0),
                    is_complete: true,
                    swatch: colour,
                    stroke_width: 4.0,
                    ..DrawProps::default()
                }),
                4 => ShapeKind::Text(TextProps {
                    text: format!("Note {i}"),
                    font_size: 22.0,
                    swatch: colour,
                    ..TextProps::default()
                }),
                _ => ShapeKind::Numbered(NumberedProps {
                    value: i as u32 % 99,
                    diameter: 40.0,
                    swatch: colour,
                }),
            };
            AnnoShape::new(x, y, kind)
        })
        .collect()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn frames(app: &mut App, mut before: impl FnMut(&mut App, usize)) -> (f64, f64) {
    let canvas = canvas(app);
    let handle = app.window.into();
    let mut times = Vec::new();
    for i in 0..220 {
        before(app, i);
        app.cx.update(|cx| canvas.update(cx, |_, cx| cx.notify()));
        let took: Duration = app
            .cx
            .update_window(handle, |_, window, cx| {
                let start = Instant::now();
                window.draw(cx).clear(cx);
                start.elapsed()
            })
            .expect("window is open");
        app.settle();
        if i >= 20 {
            times.push(took.as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    (percentile(&times, 0.5), percentile(&times, 0.95))
}

#[test]
#[ignore = "a timing bench; run with --cargo-profile fast --run-ignored only"]
fn frame_cpu_with_300_shapes_stays_within_the_120_hz_budget() {
    let (mut app, session) = open(
        1400.0,
        900.0,
        opened(2560, 1600, shapes(300), Default::default()),
    );
    let still = frames(&mut app, |_, _| {});
    let viewport = session.viewport.clone();
    app.cx
        .update(|cx| viewport.update(cx, |v, cx| v.set_percent(120.0, cx)));
    let panning = frames(&mut app, |app, i| {
        let dx = if (i / 40) % 2 == 0 { 3.0 } else { -3.0 };
        app.cx
            .update(|cx| viewport.update(cx, |v, cx| v.pan_by(dx, 1.0, cx)));
    });
    eprintln!(
        "frame CPU, 300 shapes: still p50 {:.2} ms p95 {:.2} ms; panning p50 {:.2} ms p95 {:.2} ms",
        still.0, still.1, panning.0, panning.1
    );
    assert!(still.1 <= BUDGET_MS, "still p95 {:.2} ms", still.1);
    assert!(panning.1 <= BUDGET_MS, "panning p95 {:.2} ms", panning.1);
}
