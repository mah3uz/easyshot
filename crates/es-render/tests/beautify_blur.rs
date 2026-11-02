#![allow(clippy::unwrap_used)]

use es_model::{BlurMode, ProgressiveBlurSettings};
use es_render::beautify::progressive_blur::{apply, blend_preview, focus_mask, preview_levels};
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

fn active(mode: BlurMode) -> ProgressiveBlurSettings {
    ProgressiveBlurSettings {
        is_enabled: true,
        mode,
        strength: 30.0,
        focus_size: 0.2,
        falloff: 0.3,
        ..ProgressiveBlurSettings::default()
    }
}

fn red(p: &Pixmap, x: u32, y: u32) -> i32 {
    i32::from(p.pixel(x, y).unwrap().red())
}

fn swing(out: &Pixmap, src: &Pixmap, y: u32) -> i32 {
    (0..32)
        .map(|x| (red(out, x, y) - red(src, x, y)).abs())
        .max()
        .unwrap()
}

#[test]
fn zero_strength_changes_nothing() {
    let src = stripes(200, 200);
    let mut out = src.clone();
    apply(
        &mut out,
        &ProgressiveBlurSettings {
            strength: 0.0,
            ..active(BlurMode::Radial)
        },
    );
    assert_eq!(out.data(), src.data());
}

#[test]
fn a_disabled_blur_changes_nothing() {
    let src = stripes(200, 200);
    let mut out = src.clone();
    apply(
        &mut out,
        &ProgressiveBlurSettings {
            is_enabled: false,
            ..active(BlurMode::Radial)
        },
    );
    assert_eq!(out.data(), src.data());
}

#[test]
fn radial_blur_keeps_the_focus_sharp_and_blurs_the_corners() {
    let src = stripes(400, 400);
    let mut out = src.clone();
    apply(&mut out, &active(BlurMode::Radial));
    for x in 190..210 {
        assert_eq!(red(&out, x, 200), red(&src, x, 200), "focus pixel {x}");
    }
    let corner = swing(&out, &src, 5);
    assert!(corner > 100, "stripes at the corner are smeared, {corner}");
}

#[test]
fn directional_blur_follows_the_direction_of_the_band() {
    let src = stripes(400, 400);
    let mut out = src.clone();
    apply(&mut out, &active(BlurMode::Directional));
    assert_eq!(
        swing(&out, &src, 200),
        0,
        "a horizontal band through the focus stays sharp"
    );
    let (top, bottom) = (swing(&out, &src, 2), swing(&out, &src, 397));
    assert!(top > 100 && bottom > 100, "top {top} bottom {bottom}");
}

#[test]
fn focus_position_moves_the_sharp_region() {
    let settings = ProgressiveBlurSettings {
        focus_x: 0.1,
        focus_y: 0.1,
        ..active(BlurMode::Radial)
    };
    let mask = focus_mask(400, 400, &settings);
    assert_eq!(mask.data[40 * 400 + 40], 0.0);
    assert_eq!(mask.data[360 * 400 + 360], 1.0);
}

#[test]
fn preview_blend_approximates_the_full_result() {
    // 400 blurs at full resolution; 1400 is wide enough for the reduced-resolution levels.
    for size in [400, 1400] {
        preview_matches_full(size);
    }
}

fn preview_matches_full(size: u32) {
    let src = stripes(size, size);
    let settings = active(BlurMode::Radial);
    let mut full = src.clone();
    apply(&mut full, &settings);
    let levels = preview_levels(&src, &settings).unwrap();
    let mask = focus_mask(size, size, &settings);
    let preview = blend_preview(&src, &levels, &mask).unwrap();
    let total: i64 = full
        .data()
        .iter()
        .zip(preview.data())
        .map(|(a, b)| i64::from((i32::from(*a) - i32::from(*b)).abs()))
        .sum();
    let mean = total as f64 / full.data().len() as f64;
    assert!(mean < 6.0, "{size}: mean channel error {mean}");
}

#[test]
#[ignore = "timing"]
fn time_full_quality_blur_at_2560x1600() {
    let mut canvas = stripes(2560, 1600);
    let start = std::time::Instant::now();
    apply(&mut canvas, &active(BlurMode::Radial));
    eprintln!("progressive blur 2560x1600: {:?}", start.elapsed());
}
