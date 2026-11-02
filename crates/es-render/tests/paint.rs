#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList, RedactionKind};
use es_geom::{Affine, BezPath, Cap, FillRule, Join, Rect, Rgba};
use es_render::paint::{ClipShape, paint_draw_list};
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

const RED: Rgba = Rgba::new(1.0, 0.0, 0.0, 1.0);

fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((x0, y0));
    p.line_to((x1, y0));
    p.line_to((x1, y1));
    p.line_to((x0, y1));
    p.close_path();
    p
}

fn white(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    p.fill(Color::WHITE);
    p
}

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn paint(list: &DrawList, bg: &Pixmap) -> Pixmap {
    let mut out = bg.clone();
    let clip = ClipShape::page(bg.width() as f64, bg.height() as f64);
    paint_draw_list(list, &mut out, Affine::IDENTITY, clip, bg);
    out
}

fn fill(path: BezPath, color: Rgba) -> DrawItem {
    DrawItem::Fill {
        path,
        color,
        rule: FillRule::NonZero,
    }
}

fn stroke(path: BezPath, color: Rgba, width: f64) -> DrawItem {
    DrawItem::Stroke {
        path,
        color,
        width,
        cap: Cap::Round,
        join: Join::Round,
        dash: None,
    }
}

#[test]
fn fill_covers_inside_only() {
    let out = paint(
        &DrawList {
            items: vec![fill(rect_path(10.0, 10.0, 30.0, 30.0), RED)],
        },
        &white(40, 40),
    );
    assert_eq!(px(&out, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&out, 5, 5), [255, 255, 255, 255]);
    assert_eq!(px(&out, 35, 20), [255, 255, 255, 255]);
}

#[test]
fn straight_alpha_color_blends_instead_of_darkening() {
    let half = Rgba::new(1.0, 0.0, 0.0, 0.5);
    let out = paint(
        &DrawList {
            items: vec![fill(rect_path(0.0, 0.0, 40.0, 40.0), half)],
        },
        &white(40, 40),
    );
    let p = px(&out, 20, 20);
    assert_eq!(p[0], 255);
    assert!((126..=129).contains(&p[1]), "{p:?}");
}

#[test]
fn stroke_has_requested_width_with_round_caps() {
    let mut line = BezPath::new();
    line.move_to((10.0, 20.0));
    line.line_to((30.0, 20.0));
    let out = paint(
        &DrawList {
            items: vec![stroke(line, RED, 6.0)],
        },
        &white(40, 40),
    );
    assert_eq!(px(&out, 20, 17)[1], 0);
    assert_eq!(px(&out, 20, 22)[1], 0);
    assert_eq!(px(&out, 20, 15)[1], 255);
    assert_eq!(px(&out, 20, 25)[1], 255);
    assert_eq!(px(&out, 8, 20)[1], 0, "round cap extends past the endpoint");
    assert_eq!(px(&out, 6, 20)[1], 255);
}

#[test]
fn group_applies_transform_and_halves_alpha() {
    let group = DrawItem::Group {
        transform: Affine::translate((20.0, 0.0)),
        opacity: 0.5,
        items: vec![fill(rect_path(0.0, 0.0, 10.0, 10.0), RED)],
    };
    let out = paint(&DrawList { items: vec![group] }, &white(40, 40));
    assert_eq!(px(&out, 5, 5), [255, 255, 255, 255]);
    let p = px(&out, 25, 5);
    assert!((126..=129).contains(&p[1]), "{p:?}");
}

#[test]
fn nested_group_opacity_is_per_item_not_a_layer() {
    let two = vec![
        fill(rect_path(0.0, 0.0, 20.0, 20.0), RED),
        fill(rect_path(10.0, 0.0, 30.0, 20.0), RED),
    ];
    let outer = DrawItem::Group {
        transform: Affine::IDENTITY,
        opacity: 0.5,
        items: vec![DrawItem::Group {
            transform: Affine::IDENTITY,
            opacity: 0.5,
            items: two,
        }],
    };
    let out = paint(&DrawList { items: vec![outer] }, &white(40, 40));
    let single = px(&out, 5, 5)[1];
    assert!((190..=193).contains(&single), "{single}");
    let overlap = px(&out, 15, 5)[1];
    assert!(overlap < single - 40, "alpha stacks where items overlap");
}

#[test]
fn text_fills_glyphs_and_underline() {
    let item = DrawItem::Text {
        glyphs: rect_path(2.0, 2.0, 10.0, 10.0),
        color: RED,
        underline: vec![Rect::new(2.0, 14.0, 20.0, 16.0)],
    };
    let out = paint(&DrawList { items: vec![item] }, &white(40, 40));
    assert_eq!(px(&out, 5, 5)[1], 0);
    assert_eq!(px(&out, 10, 15)[1], 0);
    assert_eq!(px(&out, 25, 12)[1], 255);
}

fn spot(holes: Vec<BezPath>) -> DrawList {
    DrawList {
        items: vec![DrawItem::Spotlight { holes }],
    }
}

#[test]
fn spotlight_dims_outside_holes_and_not_inside() {
    let out = paint(
        &spot(vec![rect_path(10.0, 10.0, 30.0, 30.0)]),
        &white(40, 40),
    );
    assert_eq!(px(&out, 20, 20), [255, 255, 255, 255]);
    let dimmed = px(&out, 3, 3)[0];
    assert!((114..=117).contains(&dimmed), "{dimmed}");
}

#[test]
fn spotlight_overlapping_holes_do_not_double_dim() {
    let out = paint(
        &spot(vec![
            rect_path(5.0, 5.0, 25.0, 25.0),
            rect_path(15.0, 15.0, 35.0, 35.0),
        ]),
        &white(40, 40),
    );
    assert_eq!(px(&out, 20, 20), [255, 255, 255, 255]);
    assert_eq!(px(&out, 8, 8), [255, 255, 255, 255]);
    assert_eq!(px(&out, 30, 30), [255, 255, 255, 255]);
    assert_eq!(px(&out, 30, 8)[0], px(&out, 3, 3)[0]);
}

#[test]
fn spotlight_stays_inside_rounded_clip() {
    let bg = white(40, 40);
    let mut out = bg.clone();
    let clip = ClipShape {
        rect: Rect::new(0.0, 0.0, 40.0, 40.0),
        radii: [12.0; 4],
    };
    paint_draw_list(&spot(vec![]), &mut out, Affine::IDENTITY, clip, &bg);
    assert_eq!(
        px(&out, 0, 0),
        [255, 255, 255, 255],
        "outside the rounded corner"
    );
    assert!(px(&out, 20, 20)[0] < 130);
}

#[test]
fn redaction_samples_the_snapshot_not_the_target() {
    let mut snap = white(40, 40);
    for y in 0..40usize {
        for x in 0..20usize {
            snap.pixels_mut()[y * 40 + x] = PremultipliedColorU8::from_rgba(0, 0, 0, 255).unwrap();
        }
    }
    let list = DrawList {
        items: vec![DrawItem::Redaction {
            rect: Rect::new(10.0, 10.0, 30.0, 30.0),
            rotation: 0.0,
            mirrored: false,
            kind: RedactionKind::Blur,
            density: 0.5,
        }],
    };
    let mut target = white(40, 40);
    paint_draw_list(
        &list,
        &mut target,
        Affine::IDENTITY,
        ClipShape::page(40.0, 40.0),
        &snap,
    );
    let mid = px(&target, 19, 20)[0];
    assert!(mid > 10 && mid < 245, "edge is smeared: {mid}");
    assert_eq!(
        px(&target, 5, 20),
        [255, 255, 255, 255],
        "outside the rect untouched"
    );
}

#[test]
fn picture_of_every_item_kind() {
    let mut bg = white(640, 360);
    for y in 0..360usize {
        for x in 0..640usize {
            if (x / 16 + y / 16) % 2 == 0 {
                bg.pixels_mut()[y * 640 + x] =
                    PremultipliedColorU8::from_rgba(40, 90, 160, 255).unwrap();
            }
        }
    }
    let mut curve = BezPath::new();
    curve.move_to((40.0, 100.0));
    curve.curve_to((120.0, 0.0), (200.0, 140.0), (280.0, 40.0));
    let items = vec![
        DrawItem::Redaction {
            rect: Rect::new(20.0, 20.0, 160.0, 100.0),
            rotation: 0.2,
            mirrored: false,
            kind: RedactionKind::Pixelate,
            density: 0.5,
        },
        DrawItem::Redaction {
            rect: Rect::new(200.0, 20.0, 340.0, 100.0),
            rotation: 0.0,
            mirrored: false,
            kind: RedactionKind::Blur,
            density: 0.6,
        },
        DrawItem::Spotlight {
            holes: vec![rect_path(360.0, 20.0, 520.0, 140.0)],
        },
        DrawItem::Group {
            transform: Affine::translate((60.0, 150.0)),
            opacity: 0.8,
            items: vec![
                fill(
                    rect_path(0.0, 0.0, 120.0, 60.0),
                    Rgba::new(1.0, 0.8, 0.0, 1.0),
                ),
                stroke(curve, Rgba::new(0.9, 0.1, 0.2, 1.0), 8.0),
            ],
        },
        DrawItem::Text {
            glyphs: rect_path(380.0, 200.0, 400.0, 240.0),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            underline: vec![Rect::new(380.0, 246.0, 460.0, 249.0)],
        },
    ];
    let out = paint(&DrawList { items }, &bg);
    if let Ok(path) = std::env::var("ES_PAINT_PNG") {
        out.save_png(path).unwrap();
    }
    assert_ne!(px(&out, 90, 60), px(&bg, 90, 60));
}

#[test]
fn three_hundred_shapes_over_2560x1440_paint_quickly() {
    let big = white(2560, 1440);
    let shapes: Vec<DrawItem> = (0..300)
        .map(|i| {
            let (x, y) = ((i % 30) as f64 * 80.0, (i / 30) as f64 * 130.0);
            let mut s = rect_path(0.0, 0.0, 70.0, 100.0);
            s.move_to((10.0, 10.0));
            s.line_to((60.0, 50.0));
            DrawItem::Group {
                transform: Affine::translate((x, y)),
                opacity: 0.9,
                items: vec![
                    fill(s.clone(), Rgba::new(0.2, 0.5, 0.9, 0.8)),
                    stroke(s, RED, 3.0),
                ],
            }
        })
        .collect();
    let list = DrawList { items: shapes };
    let start = std::time::Instant::now();
    let mut target = big.clone();
    paint_draw_list(
        &list,
        &mut target,
        Affine::IDENTITY,
        ClipShape::page(2560.0, 1440.0),
        &big,
    );
    let ms = start.elapsed().as_millis();
    eprintln!("300 shapes over 2560x1440: {ms} ms");
    assert!(ms < 2000, "{ms} ms");
}
