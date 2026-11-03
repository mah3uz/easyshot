#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::*;
use es_geom::PVec;
use es_model::{AnnoShape, HighlighterProps, ShapeKind};

fn marker(x: f64, y: f64) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Highlighter(HighlighterProps {
            points: vec![PVec::new(0.0, 0.0), PVec::new(600.0, 40.0)],
            swatch: swatch(1.0, 0.9, 0.1),
            stroke_width: 70.0,
        }),
    )
}

fn yellow_pixels(image: &image::RgbaImage) -> usize {
    image
        .pixels()
        .filter(|p| i32::from(p[0]) > i32::from(p[2]) + 40)
        .count()
}

#[test]
fn a_highlighter_stroke_tints_the_image_under_it_and_is_painted_once() {
    let (mut bare, _) = open(760.0, 420.0, opened(1200, 640, vec![], Default::default()));
    let plain = bare.screenshot("canvas-highlighter-none");
    drop(bare);
    let (mut app, _) = open(
        760.0,
        420.0,
        opened(1200, 640, vec![marker(200.0, 250.0)], Default::default()),
    );
    let shot = app.screenshot("canvas-highlighter");
    assert_eq!(yellow_pixels(&plain), 0);
    assert!(
        yellow_pixels(&shot) > 3000,
        "the stroke tinted {} pixels",
        yellow_pixels(&shot)
    );
    let held = canvas(&app).read_with(&app.cx, |c, _| c.highlighters().held());
    assert_eq!(held, 1);
}
