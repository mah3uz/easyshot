use crate::effects::{crop_pixmap, gaussian};
use es_geom::Rect;
use es_model::{BlurEdgeMode, BlurMode, ProgressiveBlurSettings};
use rayon::prelude::*;
use tiny_skia::{BlendMode, Pixmap, PixmapPaint, Transform};

const FULL_LEVELS: usize = 6;

/// 0 is sharp, 1 is fully blurred.
pub struct FocusMask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

pub fn render_radius(width: u32, height: u32, settings: &ProgressiveBlurSettings) -> f64 {
    (settings.strength * f64::from(width.min(height)) / 1000.0).max(0.5)
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn focus_mask(width: u32, height: u32, settings: &ProgressiveBlurSettings) -> FocusMask {
    let (w, h) = (f64::from(width), f64::from(height));
    let focus = (
        settings.focus_x.clamp(0.0, 1.0) * w,
        settings.focus_y.clamp(0.0, 1.0) * h,
    );
    let shortest = w.min(h);
    let transition = shortest * (0.10 + settings.falloff.clamp(0.0, 1.0) * 0.48);
    let size = settings.focus_size.clamp(0.0, 1.0);
    let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];

    let angle = settings.direction_degrees.to_radians();
    let normal = (-angle.sin(), angle.cos());
    let signed = |x: f64, y: f64| (x - focus.0) * normal.0 + (y - focus.1) * normal.1;

    let min_radius = shortest * 0.04;
    let max_radius = corners
        .iter()
        .map(|c| (c.0 - focus.0).hypot(c.1 - focus.1))
        .fold(min_radius, f64::max);
    let radius = min_radius + size * (max_radius - min_radius);

    let min_half = shortest * 0.025;
    let max_half = corners
        .iter()
        .map(|c| signed(c.0, c.1).abs())
        .fold(min_half, f64::max);
    let half = min_half + size * (max_half - min_half);

    let mut data = vec![0.0f32; width as usize * height as usize];
    data.par_chunks_exact_mut(width as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let py = y as f64 + 0.5;
            for (x, out) in row.iter_mut().enumerate() {
                let px = x as f64 + 0.5;
                *out = match settings.mode {
                    BlurMode::Radial => {
                        let d = (px - focus.0).hypot(py - focus.1);
                        ((d - radius) / transition).clamp(0.0, 1.0) as f32
                    }
                    BlurMode::Directional => {
                        let d = signed(px, py);
                        smooth((d - half) / transition).max(smooth((-d - half) / transition)) as f32
                    }
                };
            }
        });
    FocusMask {
        width,
        height,
        data,
    }
}

fn blend(sharp: &Pixmap, blurred: &[Pixmap], mask: &FocusMask) -> Option<Pixmap> {
    if mask.width != sharp.width() || mask.height != sharp.height() {
        return None;
    }
    let mut out = Pixmap::new(sharp.width(), sharp.height())?;
    let n = blurred.len();
    let w = sharp.width() as usize;
    let level = |i: usize| -> &[u8] {
        if i == 0 {
            sharp.data()
        } else {
            blurred[i - 1].data()
        }
    };
    out.data_mut()
        .par_chunks_exact_mut(w * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..w {
                let t = mask.data[y * w + x].clamp(0.0, 1.0) * n as f32;
                let lo = (t.floor() as usize).min(n - 1);
                let f = t - lo as f32;
                let (a, b) = (level(lo), level(lo + 1));
                let i = (y * w + x) * 4;
                for c in 0..4 {
                    let v = f32::from(a[i + c]) * (1.0 - f) + f32::from(b[i + c]) * f;
                    row[x * 4 + c] = v.round() as u8;
                }
            }
        });
    Some(out)
}

fn levels(canvas: &Pixmap, settings: &ProgressiveBlurSettings, count: usize) -> Vec<Pixmap> {
    let radius = render_radius(canvas.width(), canvas.height(), settings);
    (1..=count)
        .into_par_iter()
        .filter_map(|k| gaussian(canvas, radius * k as f64 / count as f64))
        .collect()
}

pub fn apply(canvas: &mut Pixmap, settings: &ProgressiveBlurSettings) {
    if !settings.is_active() {
        return;
    }
    let blurred = levels(canvas, settings, FULL_LEVELS);
    if blurred.len() != FULL_LEVELS {
        return;
    }
    let mask = focus_mask(canvas.width(), canvas.height(), settings);
    if let Some(out) = blend(canvas, &blurred, &mask) {
        *canvas = out;
    }
}

/// Bleed blurs the whole canvas; Clipped blurs only inside `image`, with the focus placed relative to it.
pub fn apply_scoped(canvas: &mut Pixmap, settings: &ProgressiveBlurSettings, image: Rect) {
    if settings.edge_mode == BlurEdgeMode::Bleed {
        return apply(canvas, settings);
    }
    if !settings.is_active() {
        return;
    }
    let (cw, ch) = (i64::from(canvas.width()), i64::from(canvas.height()));
    let x0 = (image.x0.floor() as i64).clamp(0, cw);
    let y0 = (image.y0.floor() as i64).clamp(0, ch);
    let x1 = (image.x1.ceil() as i64).clamp(0, cw);
    let y1 = (image.y1.ceil() as i64).clamp(0, ch);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let Some(mut region) = crop_pixmap(
        canvas,
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
    ) else {
        return;
    };
    apply(&mut region, settings);
    canvas.draw_pixmap(
        x0 as i32,
        y0 as i32,
        region.as_ref(),
        &PixmapPaint {
            blend_mode: BlendMode::Source,
            ..PixmapPaint::default()
        },
        Transform::identity(),
        None,
    );
}

/// Blurs at a third, two thirds and all of the render radius.
pub fn preview_levels(canvas: &Pixmap, settings: &ProgressiveBlurSettings) -> Option<[Pixmap; 3]> {
    levels(canvas, settings, 3).try_into().ok()
}

/// `canvas` and `mask` must have the size the levels were made at.
pub fn blend_preview(canvas: &Pixmap, levels: &[Pixmap; 3], mask: &FocusMask) -> Option<Pixmap> {
    blend(canvas, levels, mask)
}
