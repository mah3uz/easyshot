use crate::beautify::frame::rounded_path;
use crate::paint::ClipShape;
use es_geom::Rect;
use es_model::ShadowStyle;
use libblur::{
    BlurImage, BlurImageMut, ConvolutionMode, EdgeMode, EdgeMode2D, FastBlurChannels,
    GaussianBlurParams, ThreadingPolicy, gaussian_blur,
};
use tiny_skia::{BlendMode, Color, FillRule, Paint, Pixmap, PixmapPaint, Transform};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowLayer {
    pub y_offset: f64,
    pub radius: f64,
    pub alpha: f64,
}

fn y_offset_scale(style: ShadowStyle) -> f64 {
    match style {
        ShadowStyle::Soft => 0.3,
        ShadowStyle::Long => 0.9,
        ShadowStyle::Glow => 0.0,
        ShadowStyle::Crisp => 0.2,
    }
}

fn opacity_scale(style: ShadowStyle) -> f64 {
    match style {
        ShadowStyle::Soft => 1.0,
        ShadowStyle::Long => 0.85,
        ShadowStyle::Glow => 0.7,
        ShadowStyle::Crisp => 1.1,
    }
}

/// Opacity hits its ceiling early, so the slider mostly grows the drop instead of darkening it.
pub fn shadow_layer(style: ShadowStyle, strength: f64, reference_edge: f64) -> Option<ShadowLayer> {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 || reference_edge <= 0.0 {
        return None;
    }
    let radius = reference_edge * 0.17 * strength * style.radius_scale();
    let alpha = (0.08 + strength * 1.35).min(0.35) * opacity_scale(style);
    Some(ShadowLayer {
        y_offset: radius * y_offset_scale(style),
        radius,
        alpha: alpha.min(0.5),
    })
}

/// Casts the shadow only: the card interior is cut back out so a translucent border or
/// screenshot is not backed by black.
pub fn paint_shadow(target: &mut Pixmap, card: &ClipShape, layer: &ShadowLayer) {
    let sigma = layer.radius / 2.0;
    let margin = (sigma * 3.0).ceil() + 2.0;
    let r = card.rect;
    let x0 = (r.x0 - margin).floor();
    let y0 = (r.y0 - margin).floor();
    let w = (r.x1 + margin).ceil() - x0;
    let h = (r.y1 + margin + layer.y_offset).ceil() - y0;
    let Some(mut tmp) = Pixmap::new(w as u32, h as u32) else {
        return;
    };
    let Some(caster) = rounded_path(card.rect, card.radii) else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba(0.0, 0.0, 0.0, layer.alpha as f32).unwrap_or(Color::BLACK));
    let to_tmp = Transform::from_translate(-x0 as f32, -y0 as f32);
    tmp.fill_path(
        &caster,
        &paint,
        FillRule::Winding,
        to_tmp.post_translate(0.0, layer.y_offset as f32),
        None,
    );
    let Some(mut blurred) = gaussian(&tmp, sigma) else {
        return;
    };

    let outset = 1.0;
    let knockout_rect = Rect::new(r.x0 - outset, r.y0 - outset, r.x1 + outset, r.y1 + outset);
    let knockout_radii = card.radii.map(|v| if v > 0.0 { v + outset } else { 0.0 });
    if let Some(path) = rounded_path(knockout_rect, knockout_radii) {
        let clear = Paint {
            blend_mode: BlendMode::Clear,
            anti_alias: true,
            ..Paint::default()
        };
        blurred.fill_path(&path, &clear, FillRule::Winding, to_tmp, None);
    }
    target.draw_pixmap(
        x0 as i32,
        y0 as i32,
        blurred.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn gaussian(src: &Pixmap, sigma: f64) -> Option<Pixmap> {
    if sigma < 0.3 {
        return Some(src.clone());
    }
    let (w, h) = (src.width(), src.height());
    let input = BlurImage::borrow(src.data(), w, h, FastBlurChannels::Channels4);
    let mut output = BlurImageMut::alloc(w, h, FastBlurChannels::Channels4);
    gaussian_blur(
        &input,
        &mut output,
        GaussianBlurParams::new_from_sigma(sigma),
        EdgeMode2D::new(EdgeMode::Clamp),
        ThreadingPolicy::Adaptive,
        ConvolutionMode::FixedPoint,
    )
    .ok()?;
    let mut out = Pixmap::new(w, h)?;
    out.data_mut().copy_from_slice(output.projected());
    Some(out)
}
