#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList};
use es_geom::{Affine, BezPath, Rgba};
use es_render::paint::{ClipShape, paint_draw_list};
use tiny_skia::{Color, Pixmap};

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((x0, y0));
    p.line_to((x1, y0));
    p.line_to((x1, y1));
    p.line_to((x0, y1));
    p.close_path();
    p
}

fn paint(list: &DrawList) -> Pixmap {
    let mut out = Pixmap::new(60, 60).unwrap();
    out.fill(Color::WHITE);
    let snapshot = out.clone();
    paint_draw_list(
        list,
        &mut out,
        Affine::IDENTITY,
        ClipShape::page(60.0, 60.0),
        &snapshot,
    );
    out
}

fn white_text() -> DrawList {
    DrawList {
        items: vec![DrawItem::Text {
            glyphs: rect_path(20.0, 20.0, 40.0, 40.0),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            underline: vec![],
        }],
    }
}

#[test]
fn text_casts_a_soft_shadow_below_and_barely_above() {
    let out = paint(&white_text());
    let red = |x, y| i32::from(out.pixel(x, y).unwrap().red());
    let below = red(30, 41);
    assert!(
        (204..245).contains(&below),
        "shadow darkens below but never past alpha 0.2: {below}"
    );
    assert!(red(30, 17) > below, "less shadow above than below");
    assert_eq!(red(30, 30), 255, "the glyph itself stays white");
    assert_eq!(red(5, 5), 255, "far from the text nothing is touched");
}

#[test]
fn the_shadow_fades_with_the_text_opacity() {
    let group = DrawItem::Group {
        transform: Affine::IDENTITY,
        opacity: 0.0,
        items: white_text().items,
    };
    let out = paint(&DrawList { items: vec![group] });
    assert_eq!(out.pixel(30, 41).unwrap().red(), 255);
}
