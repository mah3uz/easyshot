use es_anno::draw::RedactionKind;
use es_geom::Rect;
use libblur::{
    BlurImage, BlurImageMut, ConvolutionMode, EdgeMode, EdgeMode2D, FastBlurChannels,
    GaussianBlurParams, ThreadingPolicy, gaussian_blur,
};
use rayon::prelude::*;
use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, PixmapPaint, Transform};

/// Premultiplied pixels placed at (x, y) in base coordinates; alpha is 0 outside the redaction's
/// rotated, mirrored rect.
pub struct Patch {
    pub pixmap: Pixmap,
    pub x: i32,
    pub y: i32,
}

impl Patch {
    pub fn draw_onto(&self, target: &mut Pixmap) {
        target.draw_pixmap(
            self.x,
            self.y,
            self.pixmap.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
}

pub fn blur_radius(density: f64) -> f64 {
    2.0 + density.clamp(0.0, 1.0) * 28.0
}

pub fn pixel_block(density: f64) -> f64 {
    4.0 + density.clamp(0.0, 1.0) * 36.0
}

const MIN_BLOCK_PX: f64 = 12.0;
const MIN_SIGMA_PX: f64 = 8.0;
const MAX_FLOOR_PX: f64 = 40.0;

/// Output pixels per block, never smaller than half the rect's short side (text height), so no
/// glyph survives as more than a couple of cells.
pub fn effective_block(density: f64, short_side: f64, scale: f64) -> f64 {
    let floor = (short_side * 0.5).clamp(MIN_BLOCK_PX * scale, MAX_FLOOR_PX * scale);
    (pixel_block(density) * scale).max(floor)
}

pub fn effective_sigma(density: f64, short_side: f64, scale: f64) -> f64 {
    let floor = (short_side * 0.4).clamp(MIN_SIGMA_PX * scale, MAX_FLOOR_PX * scale);
    (blur_radius(density) * scale).max(floor)
}

pub fn redaction_patch(
    base: &Pixmap,
    rect: Rect,
    rotation: f64,
    mirrored: bool,
    kind: RedactionKind,
    density: f64,
    scale: f64,
) -> Option<Patch> {
    let (w, h) = (rect.width(), rect.height());
    let short_side = w.min(h);
    let flip = if mirrored { -1.0 } else { 1.0 };
    let (sin, cos) = rotation.sin_cos();
    let corner = |lx: f64, ly: f64| {
        let lx = lx * flip;
        (rect.x0 + lx * cos - ly * sin, rect.y0 + lx * sin + ly * cos)
    };
    let quad = [
        corner(0.0, 0.0),
        corner(w, 0.0),
        corner(w, h),
        corner(0.0, h),
    ];
    let min_x = quad.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = quad.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let min_y = quad.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max_y = quad.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);

    let (bw, bh) = (base.width() as i64, base.height() as i64);
    let x0 = (min_x.floor() as i64).clamp(0, bw);
    let y0 = (min_y.floor() as i64).clamp(0, bh);
    let x1 = (max_x.ceil() as i64).clamp(0, bw);
    let y1 = (max_y.ceil() as i64).clamp(0, bh);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (pw, ph) = ((x1 - x0) as u32, (y1 - y0) as u32);

    let mut out = match kind {
        RedactionKind::Pixelate => {
            let crop = crop_pixmap(base, x0 as u32, y0 as u32, pw, ph)?;
            pixelate(
                &crop,
                effective_block(density, short_side, scale).round().max(1.0) as u32,
            )?
        }
        RedactionKind::Blur => {
            let radius = effective_sigma(density, short_side, scale);
            let pad = (radius * 2.0).ceil() as i64;
            let (px0, py0) = ((x0 - pad).max(0), (y0 - pad).max(0));
            let (px1, py1) = ((x1 + pad).min(bw), (y1 + pad).min(bh));
            let padded = crop_pixmap(
                base,
                px0 as u32,
                py0 as u32,
                (px1 - px0) as u32,
                (py1 - py0) as u32,
            )?;
            let blurred = gaussian(&padded, radius)?;
            crop_pixmap(&blurred, (x0 - px0) as u32, (y0 - py0) as u32, pw, ph)?
        }
    };

    let mut pb = PathBuilder::new();
    for (i, p) in quad.iter().enumerate() {
        let (x, y) = ((p.0 - x0 as f64) as f32, (p.1 - y0 as f64) as f32);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    if rotation != 0.0 {
        let mut mask = Mask::new(pw, ph)?;
        mask.fill_path(
            &pb.finish()?,
            FillRule::Winding,
            true,
            Transform::identity(),
        );
        out.data_mut()
            .par_chunks_exact_mut(4)
            .zip(mask.data().par_iter())
            .for_each(|(px, &cov)| {
                let c = cov as u32;
                for ch in px.iter_mut() {
                    *ch = ((*ch as u32 * c + 127) / 255) as u8;
                }
            });
    }
    Some(Patch {
        pixmap: out,
        x: x0 as i32,
        y: y0 as i32,
    })
}

pub(crate) fn crop_pixmap(src: &Pixmap, x: u32, y: u32, w: u32, h: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(w, h)?;
    let sw = src.width() as usize;
    let row_len = w as usize * 4;
    for row in 0..h as usize {
        let s = ((y as usize + row) * sw + x as usize) * 4;
        out.data_mut()[row * row_len..(row + 1) * row_len]
            .copy_from_slice(&src.data()[s..s + row_len]);
    }
    Some(out)
}

pub(crate) fn gaussian(src: &Pixmap, sigma: f64) -> Option<Pixmap> {
    gaussian_with(src, sigma, ConvolutionMode::Exact)
}

pub(crate) fn gaussian_with(src: &Pixmap, sigma: f64, mode: ConvolutionMode) -> Option<Pixmap> {
    let (w, h) = (src.width(), src.height());
    let input = BlurImage::borrow(src.data(), w, h, FastBlurChannels::Channels4);
    let mut output = BlurImageMut::alloc(w, h, FastBlurChannels::Channels4);
    gaussian_blur(
        &input,
        &mut output,
        GaussianBlurParams::new_from_sigma(sigma.max(0.5)),
        EdgeMode2D::new(EdgeMode::Clamp),
        ThreadingPolicy::Adaptive,
        mode,
    )
    .ok()?;
    let mut out = Pixmap::new(w, h)?;
    out.data_mut().copy_from_slice(output.projected());
    Some(out)
}

fn pixelate(src: &Pixmap, block: u32) -> Option<Pixmap> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let sw = (w / block as usize).max(1);
    let sh = (h / block as usize).max(1);
    let data = src.data();
    let mut small = vec![[0u8; 4]; sw * sh];
    small.par_iter_mut().enumerate().for_each(|(i, cell)| {
        let (cx, cy) = (i % sw, i / sw);
        let (xa, ya) = (cx * w / sw, cy * h / sh);
        let xb = ((cx + 1) * w / sw).max(xa + 1);
        let yb = ((cy + 1) * h / sh).max(ya + 1);
        let mut sum = [0u64; 4];
        for y in ya..yb {
            for x in xa..xb {
                for (c, s) in sum.iter_mut().enumerate() {
                    *s += data[(y * w + x) * 4 + c] as u64;
                }
            }
        }
        let n = ((xb - xa) * (yb - ya)) as u64;
        for (c, s) in sum.iter().enumerate() {
            cell[c] = ((s + n / 2) / n) as u8;
        }
    });
    let mut out = Pixmap::new(w as u32, h as u32)?;
    out.data_mut()
        .par_chunks_exact_mut(w * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let sy = y * sh / h;
            for x in 0..w {
                row[x * 4..x * 4 + 4].copy_from_slice(&small[sy * sw + x * sw / w]);
            }
        });
    Some(out)
}
