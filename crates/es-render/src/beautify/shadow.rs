use crate::beautify::blit::mul;
use crate::beautify::frame::rounded_path;
use crate::paint::ClipShape;
use es_geom::Rect;
use es_model::ShadowStyle;
use libblur::{
    BlurImage, BlurImageMut, ConvolutionMode, EdgeMode, EdgeMode2D, FastBlurChannels,
    GaussianBlurParams, ThreadingPolicy, gaussian_blur,
};
use rayon::prelude::*;
use tiny_skia::{FillRule, Mask, Pixmap, Transform};

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
    // A blur this wide has no detail finer than a few pixels, so it is cast at reduced resolution.
    let step = (sigma / 3.0).floor().clamp(1.0, 4.0) as u32;
    let margin = (sigma * 3.0).ceil() + 2.0;
    let r = card.rect;
    let x0 = (r.x0 - margin).floor();
    let y0 = (r.y0 - margin).floor();
    let w = ((r.x1 + margin).ceil() - x0) as u32;
    let h = ((r.y1 + margin + layer.y_offset).ceil() - y0) as u32;
    let (sw, sh) = (w.div_ceil(step), h.div_ceil(step));
    let (Some(mut small), Some(caster)) = (Mask::new(sw, sh), rounded_path(card.rect, card.radii))
    else {
        return;
    };
    let k = 1.0 / step as f32;
    let to_small =
        Transform::from_translate(-x0 as f32, (layer.y_offset - y0) as f32).post_scale(k, k);
    small.fill_path(&caster, FillRule::Winding, true, to_small);
    let Some(blurred) = gaussian_plane(small.data(), sw, sh, sigma / f64::from(step)) else {
        return;
    };

    let outset = 1.0;
    let knockout_rect = Rect::new(r.x0 - outset, r.y0 - outset, r.x1 + outset, r.y1 + outset);
    let knockout_radii = card.radii.map(|v| if v > 0.0 { v + outset } else { 0.0 });
    let kx0 = knockout_rect.x0.floor();
    let ky0 = knockout_rect.y0.floor();
    let kw = (knockout_rect.x1.ceil() - kx0) as u32;
    let kh = (knockout_rect.y1.ceil() - ky0) as u32;
    let (Some(mut knockout), Some(path)) = (
        Mask::new(kw, kh),
        rounded_path(knockout_rect, knockout_radii),
    ) else {
        return;
    };
    let to_knockout = Transform::from_translate(-kx0 as f32, -ky0 as f32);
    knockout.fill_path(&path, FillRule::Winding, true, to_knockout);

    let cast = Cast {
        blurred: &blurred,
        small: (sw as usize, sh as usize),
        step: step as f32,
        knockout: &knockout,
        knockout_at: ((kx0 - x0) as i64, (ky0 - y0) as i64),
        alpha: layer.alpha.clamp(0.0, 1.0) as f32,
    };
    let (x0, y0) = (x0 as i64, y0 as i64);
    let (tw, th) = (i64::from(target.width()), i64::from(target.height()));
    let (cx0, cx1) = (x0.max(0), (x0 + i64::from(w)).min(tw));
    let (cy0, cy1) = (y0.max(0), (y0 + i64::from(h)).min(th));
    if cx1 <= cx0 || cy1 <= cy0 {
        return;
    }
    let columns: Vec<Tap> = (cx0..cx1).map(|x| cast.tap(x - x0, sw)).collect();
    let stride = tw as usize * 4;
    target.data_mut()[cy0 as usize * stride..cy1 as usize * stride]
        .par_chunks_exact_mut(stride)
        .enumerate()
        .for_each(|(row, dst)| {
            let v = cy0 + row as i64 - y0;
            let dst = &mut dst[cx0 as usize * 4..cx1 as usize * 4];
            cast.row(dst, v, cx0 - x0, &columns);
        });
}

#[derive(Clone, Copy)]
struct Tap {
    lo: usize,
    hi: usize,
    f: f32,
}

struct Cast<'a> {
    blurred: &'a [u8],
    small: (usize, usize),
    step: f32,
    knockout: &'a Mask,
    knockout_at: (i64, i64),
    alpha: f32,
}

impl Cast<'_> {
    /// Bilinear taps for full-resolution position `u` on an axis `len` small pixels long.
    fn tap(&self, u: i64, len: u32) -> Tap {
        let p = ((u as f32 + 0.5) / self.step - 0.5).max(0.0);
        let lo = (p.floor() as usize).min(len as usize - 1);
        Tap {
            lo,
            hi: (lo + 1).min(len as usize - 1),
            f: p - lo as f32,
        }
    }

    fn row(&self, dst: &mut [u8], v: i64, u0: i64, columns: &[Tap]) {
        let ty = self.tap(v, self.small.1 as u32);
        let w = self.small.0;
        let (top, bottom) = (
            &self.blurred[ty.lo * w..(ty.lo + 1) * w],
            &self.blurred[ty.hi * w..(ty.hi + 1) * w],
        );
        let (kw, kh) = (self.knockout.width() as i64, self.knockout.height() as i64);
        let ky = v - self.knockout_at.1;
        let knock = (0..kh).contains(&ky).then(|| {
            let start = (ky * kw) as usize;
            &self.knockout.data()[start..start + kw as usize]
        });
        for (i, (d, tx)) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(columns)
            .enumerate()
        {
            let kx = u0 + i as i64 - self.knockout_at.0;
            let cut = match knock {
                Some(k) if (0..kw).contains(&kx) => k[kx as usize],
                _ => 0,
            };
            if cut == 255 {
                continue;
            }
            let lerp = |a: u8, b: u8| f32::from(a) + (f32::from(b) - f32::from(a)) * tx.f;
            let t = lerp(top[tx.lo], top[tx.hi]);
            let b = lerp(bottom[tx.lo], bottom[tx.hi]);
            let cover = t + (b - t) * ty.f;
            let s = (cover * self.alpha * f32::from(255 - cut) / 255.0).round() as u8;
            if s == 0 {
                continue;
            }
            let keep = 255 - s;
            for c in &mut d[..3] {
                *c = mul(*c, keep);
            }
            d[3] = s.saturating_add(mul(d[3], keep));
        }
    }
}

fn gaussian_plane(src: &[u8], w: u32, h: u32, sigma: f64) -> Option<Vec<u8>> {
    if sigma < 0.3 {
        return Some(src.to_vec());
    }
    let input = BlurImage::borrow(src, w, h, FastBlurChannels::Plane);
    let mut output = BlurImageMut::alloc(w, h, FastBlurChannels::Plane);
    gaussian_blur(
        &input,
        &mut output,
        GaussianBlurParams::new_from_sigma(sigma),
        EdgeMode2D::new(EdgeMode::Clamp),
        ThreadingPolicy::Adaptive,
        ConvolutionMode::FixedPoint,
    )
    .ok()?;
    Some(output.projected().to_vec())
}
