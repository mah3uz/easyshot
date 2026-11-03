use crate::errors::{Error, Result};
use crate::pipeline::pixmap;
use es_anno::draw::{DrawItem, DrawList, RedactionKind};
use es_geom::Size;
use es_model::BackgroundSettings;
use es_model::platform::{Frame, Transform};
use es_ocr::{OcrOptions, Recognized};
use es_render::beautify::compose_with_wallpaper;
use image::RgbaImage;
use std::sync::Arc;

const DENSITY: f64 = 0.8;

#[derive(Debug, Clone)]
pub struct Redacted {
    pub image: RgbaImage,
    pub regions: usize,
}

/// Covers what looks like a secret in `image` with the renderer's pixelation. `NotInstalled` from
/// the recognizer reaches the caller, which decides whether an unredacted image may go on.
pub fn redact_secrets(
    image: &RgbaImage,
    languages: &[String],
    recognize: &dyn Fn(&RgbaImage, &OcrOptions) -> std::result::Result<Recognized, es_ocr::Error>,
) -> Result<Redacted> {
    let options = OcrOptions {
        languages: languages.to_vec(),
        ..OcrOptions::default()
    };
    let found = recognize(image, &options).map_err(|e| match e {
        es_ocr::Error::NotInstalled => Error::TesseractMissing,
        es_ocr::Error::Cancelled => Error::Cancelled,
        other => Error::Protocol(other.to_string()),
    })?;
    let size = Size::new(f64::from(image.width()), f64::from(image.height()));
    let regions = es_ocr::redact::find(&found.words, size);
    if regions.is_empty() {
        return Ok(Redacted {
            image: image.clone(),
            regions: 0,
        });
    }
    let list = DrawList {
        items: regions
            .iter()
            .map(|rect| DrawItem::Redaction {
                rect: *rect,
                rotation: 0.0,
                mirrored: false,
                kind: RedactionKind::Pixelate,
                density: DENSITY,
            })
            .collect(),
    };
    let covered =
        compose_with_wallpaper(&pixmap(image)?, &list, &BackgroundSettings::default(), None)
            .ok_or_else(|| Error::Internal("the redacted image could not be allocated".into()))?;
    let image = RgbaImage::from_raw(covered.width(), covered.height(), covered.data().to_vec())
        .ok_or_else(|| Error::Internal("the redacted image has the wrong size".into()))?;
    Ok(Redacted {
        image,
        regions: regions.len(),
    })
}

/// An upright frame with `image`'s pixels and the geometry of `template`.
pub fn frame_of(template: &Frame, image: &RgbaImage) -> Frame {
    let bgra: Vec<u8> = image
        .as_raw()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0], 255])
        .collect();
    Frame {
        size_px: (image.width(), image.height()),
        bgra: Arc::from(bgra),
        transform: Transform::Normal,
        ..template.clone()
    }
}
