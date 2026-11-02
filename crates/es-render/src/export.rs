use crate::beautify::compose_with_wallpaper;
use crate::beautify::wallpaper::decode_wallpaper;
use crate::encode::{EncodeError, OutputFormat, PngProfile, encode};
use es_anno::draw::DrawList;
use es_model::{BackgroundSettings, BackgroundStyle};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tiny_skia::Pixmap;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("the composed image could not be allocated")]
    Compose,
    #[error(transparent)]
    Encode(#[from] EncodeError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Clipboard, drag and deck staging: speed over size.
    Transient,
    /// Save and Export: smaller files.
    Keep,
}

impl Purpose {
    pub fn png_profile(self) -> PngProfile {
        match self {
            Self::Transient => PngProfile::Fast,
            Self::Keep => PngProfile::Small,
        }
    }
}

/// Remembers the last composed image; the same base, annotations, background and wallpaper compose once.
#[derive(Default)]
pub struct ExportCache {
    entry: Option<([u8; 32], Arc<Pixmap>)>,
    hits: u32,
}

impl ExportCache {
    pub fn hits(&self) -> u32 {
        self.hits
    }

    pub fn composed(
        &mut self,
        base: &Pixmap,
        annotations: &DrawList,
        settings: &BackgroundSettings,
        wallpaper: Option<&[u8]>,
    ) -> Result<Arc<Pixmap>, ExportError> {
        let key = cache_key(base, annotations, settings, wallpaper);
        if let Some((k, pixmap)) = &self.entry
            && *k == key
        {
            self.hits += 1;
            return Ok(pixmap.clone());
        }
        let decoded = match (&settings.style, wallpaper) {
            (BackgroundStyle::Wallpaper { .. }, Some(bytes)) => decode_wallpaper(bytes),
            _ => None,
        };
        let pixmap = Arc::new(
            compose_with_wallpaper(base, annotations, settings, decoded.as_ref())
                .ok_or(ExportError::Compose)?,
        );
        self.entry = Some((key, pixmap.clone()));
        Ok(pixmap)
    }

    pub fn export(
        &mut self,
        base: &Pixmap,
        annotations: &DrawList,
        settings: &BackgroundSettings,
        wallpaper: Option<&[u8]>,
        format: OutputFormat,
    ) -> Result<Vec<u8>, ExportError> {
        let composed = self.composed(base, annotations, settings, wallpaper)?;
        Ok(encode(&composed, format)?)
    }
}

/// Composes and encodes without remembering anything.
pub fn export(
    base: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    wallpaper: Option<&[u8]>,
    format: OutputFormat,
) -> Result<Vec<u8>, ExportError> {
    ExportCache::default().export(base, annotations, settings, wallpaper, format)
}

fn cache_key(
    base: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    wallpaper: Option<&[u8]>,
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(base.width().to_le_bytes());
    h.update(base.height().to_le_bytes());
    h.update(base.data());
    h.update(format!("{annotations:?}"));
    h.update(format!("{settings:?}"));
    match wallpaper {
        Some(bytes) => {
            h.update([1]);
            h.update(bytes);
        }
        None => h.update([0]),
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(h.finalize().as_slice());
    key
}
