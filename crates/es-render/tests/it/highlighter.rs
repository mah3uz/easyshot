#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList};
use es_geom::{Affine, BezPath, Rgba};
use es_render::paint::{ClipShape, paint_draw_list};
use tiny_skia::{Color, Pixmap};

fn filled(color: Color) -> Pixmap {
    let mut p = Pixmap::new(40, 20).unwrap();
    p.fill(color);
    p
}

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn list() -> DrawList {
    let mut line = BezPath::new();
    line.move_to((4.0, 10.0));
    line.line_to((36.0, 10.0));
    DrawList {
        items: vec![DrawItem::Highlighter {
            path: line,
            width: 10.0,
            color: Rgba::new(1.0, 0.9, 0.0, 0.4),
        }],
    }
}

fn paint(list: &DrawList, bg: &Pixmap) -> Pixmap {
    let mut out = bg.clone();
    let clip = ClipShape::page(f64::from(bg.width()), f64::from(bg.height()));
    paint_draw_list(list, &mut out, Affine::IDENTITY, clip, bg);
    out
}

#[test]
fn tints_white_and_leaves_black_dark() {
    let on_white = paint(&list(), &filled(Color::WHITE));
    let [r, g, b, _] = px(&on_white, 20, 10);
    assert!(
        r > 240 && g > 230 && b < 200,
        "tinted yellow, got {r} {g} {b}"
    );
    assert_eq!(px(&on_white, 20, 2), [255, 255, 255, 255]);

    let on_black = paint(&list(), &filled(Color::BLACK));
    let [r, g, b, _] = px(&on_black, 20, 10);
    assert!(
        r < 6 && g < 6 && b < 6,
        "black stays black, got {r} {g} {b}"
    );
}

#[test]
fn the_editor_patch_paints_the_same_pixels_as_export() {
    let base = filled(Color::WHITE);
    let DrawItem::Highlighter { path, width, color } = &list().items[0] else {
        unreachable!()
    };
    let patch = es_render::effects::highlighter_patch(&base, path, *width, *color).unwrap();
    let mut via_patch = base.clone();
    patch.draw_onto(&mut via_patch);
    let direct = paint(&list(), &base);
    let worst = via_patch
        .data()
        .iter()
        .zip(direct.data())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(worst <= 2, "off by {worst}");
}
