#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::*;
use es_anno::{BuildContext, DrawItem, build_draw_list};
use es_geom::{Affine, FillRule};
use es_render::paint::{ClipShape, paint_draw_list};
use image::RgbaImage;
use kurbo::Shape;
use tiny_skia::{IntSize, Pixmap};

/// Glyphs as plain fills: the text shadow is left out of the comparison.
fn without_text_shadow(items: Vec<DrawItem>) -> Vec<DrawItem> {
    items
        .into_iter()
        .flat_map(|item| match item {
            DrawItem::Text {
                glyphs,
                color,
                underline,
            } => {
                let mut out = vec![DrawItem::Fill {
                    path: glyphs,
                    color,
                    rule: FillRule::NonZero,
                }];
                out.extend(underline.iter().map(|r| DrawItem::Fill {
                    path: r.to_path(0.1),
                    color,
                    rule: FillRule::NonZero,
                }));
                out
            }
            DrawItem::Group {
                transform,
                opacity,
                items,
            } => vec![DrawItem::Group {
                transform,
                opacity,
                items: without_text_shadow(items),
            }],
            other => vec![other],
        })
        .collect()
}

fn lab(p: [u8; 4]) -> [f64; 3] {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(p[0]), lin(p[1]), lin(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn delta_e(a: [u8; 4], b: [u8; 4]) -> f64 {
    let (p, q) = (lab(a), lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

#[test]
fn the_canvas_at_100_percent_matches_the_export_pixel_for_pixel() {
    let (w, h) = (1200u32, 640u32);
    let (mut app, session) = open(
        700.0,
        400.0,
        opened(w, h, every_kind(false, true), Default::default()),
    );
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(100.0, cx))
    });
    let shot = app.screenshot("parity-canvas");
    let origin = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    let (ox, oy) = ((origin.x * 2.0) as u32, (origin.y * 2.0) as u32);
    assert_eq!(
        (origin.x * 2.0).fract(),
        0.0,
        "the image sits on whole device pixels"
    );

    let list = app.cx.update(|cx| {
        let doc = session.doc.read(cx);
        build_draw_list(doc.editor().doc(), &BuildContext::new(&**TEXT))
    });
    let mut list = list;
    list.items = without_text_shadow(list.items);
    let source = base(w, h);
    let snapshot =
        Pixmap::from_vec(source.clone().into_raw(), IntSize::from_wh(w, h).unwrap()).unwrap();
    let mut export = snapshot.clone();
    paint_draw_list(
        &list,
        &mut export,
        Affine::IDENTITY,
        ClipShape::page(f64::from(w), f64::from(h)),
        &snapshot,
    );

    let mut diff = RgbaImage::new(w, h);
    let mut off = 0usize;
    for y in 0..h {
        for x in 0..w {
            let c = shot.get_pixel(ox + x, oy + y).0;
            let i = ((y * w + x) * 4) as usize;
            let e: [u8; 4] = export.data()[i..i + 4].try_into().unwrap();
            if delta_e(c, e) > 3.0 {
                off += 1;
                diff.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            } else {
                diff.put_pixel(x, y, image::Rgba([c[0] / 3, c[1] / 3, c[2] / 3, 255]));
            }
        }
    }
    let share = off as f64 / f64::from(w * h);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    diff.save(dir.join("parity-diff.png")).unwrap();
    eprintln!(
        "parity: {off} of {} pixels ({:.3} %) differ by more than dE 3",
        w * h,
        share * 100.0
    );
    assert!(share <= 0.02, "{:.3} % of pixels differ", share * 100.0);
}
