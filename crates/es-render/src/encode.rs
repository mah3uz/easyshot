use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use rayon::prelude::*;
use tiny_skia::Pixmap;

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("image has no pixels")]
    Empty,
    #[error(transparent)]
    Image(#[from] image::ImageError),
}

/// Fast is for clipboard, drag and deck staging; Small is for files the user keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PngProfile {
    Fast,
    Small,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png(PngProfile),
    Jpeg { quality_percent: u8 },
}

pub fn encode(pixmap: &Pixmap, format: OutputFormat) -> Result<Vec<u8>, EncodeError> {
    match format {
        OutputFormat::Png(profile) => encode_png(pixmap, profile),
        OutputFormat::Jpeg { quality_percent } => encode_jpeg(pixmap, quality_percent),
    }
}

pub fn encode_png(pixmap: &Pixmap, profile: PngProfile) -> Result<Vec<u8>, EncodeError> {
    let (w, h) = (pixmap.width(), pixmap.height());
    if w == 0 || h == 0 {
        return Err(EncodeError::Empty);
    }
    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    rgba.par_chunks_exact_mut(4)
        .zip(pixmap.data().par_chunks_exact(4))
        .for_each(|(out, px)| {
            let a = u32::from(px[3]);
            for c in 0..3 {
                out[c] = ((u32::from(px[c]) * 255 + a / 2).checked_div(a))
                    .map_or(0, |v| v.min(255) as u8);
            }
            out[3] = px[3];
        });
    let (compression, filter) = match profile {
        PngProfile::Fast => (CompressionType::Fast, FilterType::Adaptive),
        PngProfile::Small => (CompressionType::Default, FilterType::Adaptive),
    };
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, compression, filter).write_image(
        &rgba,
        w,
        h,
        ExtendedColorType::Rgba8,
    )?;
    Ok(out)
}

/// JPEG has no alpha: transparent areas become white.
pub fn encode_jpeg(pixmap: &Pixmap, quality_percent: u8) -> Result<Vec<u8>, EncodeError> {
    let (w, h) = (pixmap.width(), pixmap.height());
    if w == 0 || h == 0 {
        return Err(EncodeError::Empty);
    }
    let mut rgb = vec![0u8; w as usize * h as usize * 3];
    rgb.par_chunks_exact_mut(3)
        .zip(pixmap.data().par_chunks_exact(4))
        .for_each(|(out, px)| {
            let uncovered = 255 - px[3];
            for c in 0..3 {
                out[c] = px[c].saturating_add(uncovered);
            }
        });
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality_percent.clamp(1, 100)).write_image(
        &rgb,
        w,
        h,
        ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}
