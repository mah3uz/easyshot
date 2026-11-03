#![allow(clippy::unwrap_used)]

use es_geom::Rect;
use es_model::{BlurEdgeMode, BlurMode, ProgressiveBlurSettings};
use es_render::beautify::progressive_blur::apply_scoped;
use tiny_skia::Pixmap;

fn stripes(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    let data = p.data_mut();
    for i in 0..(w * h) as usize {
        let v = if (i as u32 % w / 4).is_multiple_of(2) {
            255
        } else {
            0
        };
        data[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, 255]);
    }
    p
}

fn swing(out: &Pixmap, src: &Pixmap, y: u32) -> i32 {
    (0..400)
        .map(|x| {
            let r = |p: &Pixmap| i32::from(p.pixel(x, y).unwrap().red());
            (r(out) - r(src)).abs()
        })
        .max()
        .unwrap()
}

fn settings(edge_mode: BlurEdgeMode) -> ProgressiveBlurSettings {
    ProgressiveBlurSettings {
        is_enabled: true,
        edge_mode,
        mode: BlurMode::Radial,
        strength: 30.0,
        focus_x: 0.0,
        focus_y: 0.0,
        focus_size: 0.0,
        falloff: 0.3,
        ..ProgressiveBlurSettings::default()
    }
}

#[test]
fn clipped_blur_leaves_everything_outside_the_image_sharp() {
    let src = stripes(400, 400);
    let image = Rect::new(100.0, 100.0, 300.0, 300.0);
    let mut out = src.clone();
    apply_scoped(&mut out, &settings(BlurEdgeMode::Clipped), image);
    assert_eq!(swing(&out, &src, 50), 0, "above the image");
    assert_eq!(swing(&out, &src, 350), 0, "below the image");
    assert!(swing(&out, &src, 290) > 100, "inside the image");
}

#[test]
fn bleed_blur_reaches_the_whole_canvas() {
    let src = stripes(400, 400);
    let image = Rect::new(100.0, 100.0, 300.0, 300.0);
    let mut out = src.clone();
    apply_scoped(&mut out, &settings(BlurEdgeMode::Bleed), image);
    assert!(swing(&out, &src, 390) > 100);
}
