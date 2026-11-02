#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_geom::Rect;
use es_model::ShadowStyle;
use es_render::beautify::blit;
use es_render::beautify::frame::rounded_path;
use es_render::beautify::shadow::{ShadowLayer, paint_shadow, shadow_layer};
use es_render::paint::ClipShape;
use libblur::{
    BlurImage, BlurImageMut, ConvolutionMode, EdgeMode, EdgeMode2D, FastBlurChannels,
    GaussianBlurParams, ThreadingPolicy, gaussian_blur,
};
use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, Mask, Paint, Pattern, Pixmap, PixmapPaint,
    SpreadMode, Transform,
};

fn noise(w: u32, h: u32, seed: u32, translucent: bool) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    let mut x = seed;
    for px in p.data_mut().as_chunks_mut::<4>().0 {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let b = x.to_le_bytes();
        let a = if translucent { b[3] } else { 255 };
        let pre = |c: u8| ((u16::from(c) * u16::from(a) + 127) / 255) as u8;
        *px = [pre(b[0]), pre(b[1]), pre(b[2]), a];
    }
    p
}

fn max_diff(a: &Pixmap, b: &Pixmap) -> u8 {
    a.data()
        .iter()
        .zip(b.data())
        .map(|(x, y)| x.abs_diff(*y))
        .max()
        .unwrap()
}

#[test]
fn the_blit_matches_tiny_skia_source_over_including_clipped_edges() {
    let src = noise(90, 70, 7, true);
    for (x, y) in [(10, 12), (-30, 5), (60, -20), (-5, -5)] {
        let mut ours = noise(120, 80, 3, false);
        let mut theirs = ours.clone();
        blit::over(&mut ours, &src, x, y, None);
        theirs.draw_pixmap(
            x,
            y,
            src.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        assert!(max_diff(&ours, &theirs) <= 1, "offset ({x}, {y})");
    }
}

#[test]
fn a_masked_blit_matches_a_rounded_image_fill() {
    let src = noise(200, 120, 11, false);
    let rect = Rect::new(20.0, 30.0, 220.0, 150.0);
    let path = rounded_path(rect, [24.0; 4]).unwrap();
    let mut mask = Mask::new(200, 120).unwrap();
    mask.fill_path(
        &path,
        FillRule::Winding,
        true,
        Transform::from_translate(-20.0, -30.0),
    );
    let mut ours = noise(260, 180, 5, false);
    let mut theirs = ours.clone();
    blit::over(&mut ours, &src, 20, 30, Some(&mask));
    let paint = Paint {
        anti_alias: true,
        shader: Pattern::new(
            src.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Nearest,
            1.0,
            Transform::from_translate(20.0, 30.0),
        ),
        ..Paint::default()
    };
    theirs.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    assert!(max_diff(&ours, &theirs) <= 2);
}

/// The shadow cast at full resolution on all four channels, as tiny-skia and libblur draw it.
fn full_resolution_shadow(target: &mut Pixmap, card: &ClipShape, layer: &ShadowLayer) {
    let sigma = layer.radius / 2.0;
    let margin = (sigma * 3.0).ceil() + 2.0;
    let r = card.rect;
    let (x0, y0) = ((r.x0 - margin).floor(), (r.y0 - margin).floor());
    let w = ((r.x1 + margin).ceil() - x0) as u32;
    let h = ((r.y1 + margin + layer.y_offset).ceil() - y0) as u32;
    let mut tmp = Pixmap::new(w, h).unwrap();
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba(0.0, 0.0, 0.0, layer.alpha as f32).unwrap());
    let to_tmp = Transform::from_translate(-x0 as f32, -y0 as f32);
    tmp.fill_path(
        &rounded_path(r, card.radii).unwrap(),
        &paint,
        FillRule::Winding,
        to_tmp.post_translate(0.0, layer.y_offset as f32),
        None,
    );
    let mut blurred = Pixmap::new(w, h).unwrap();
    if sigma >= 0.3 {
        let input = BlurImage::borrow(tmp.data(), w, h, FastBlurChannels::Channels4);
        let mut output = BlurImageMut::alloc(w, h, FastBlurChannels::Channels4);
        gaussian_blur(
            &input,
            &mut output,
            GaussianBlurParams::new_from_sigma(sigma),
            EdgeMode2D::new(EdgeMode::Clamp),
            ThreadingPolicy::Single,
            ConvolutionMode::Exact,
        )
        .unwrap();
        blurred.data_mut().copy_from_slice(output.projected());
    } else {
        blurred = tmp;
    }
    let knockout = Rect::new(r.x0 - 1.0, r.y0 - 1.0, r.x1 + 1.0, r.y1 + 1.0);
    let radii = card.radii.map(|v| if v > 0.0 { v + 1.0 } else { 0.0 });
    let clear = Paint {
        blend_mode: BlendMode::Clear,
        anti_alias: true,
        ..Paint::default()
    };
    let path = rounded_path(knockout, radii).unwrap();
    blurred.fill_path(&path, &clear, FillRule::Winding, to_tmp, None);
    target.draw_pixmap(
        x0 as i32,
        y0 as i32,
        blurred.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

#[test]
fn the_reduced_resolution_shadow_matches_a_full_resolution_cast() {
    let card = ClipShape {
        rect: Rect::new(160.0, 120.0, 560.0, 380.0),
        radii: [18.0; 4],
    };
    for (style, strength) in [
        (ShadowStyle::Soft, 1.0),
        (ShadowStyle::Long, 0.5),
        (ShadowStyle::Crisp, 0.05),
    ] {
        let layer = shadow_layer(style, strength, 260.0).unwrap();
        let mut ours = Pixmap::new(720, 560).unwrap();
        ours.fill(Color::from_rgba8(200, 210, 220, 255));
        let mut reference = ours.clone();
        paint_shadow(&mut ours, &card, &layer);
        full_resolution_shadow(&mut reference, &card, &layer);
        let diff = max_diff(&ours, &reference);
        assert!(diff <= 3, "{style:?} at {strength}: off by {diff}");
    }
}
