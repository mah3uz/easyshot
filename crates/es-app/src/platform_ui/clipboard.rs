use super::ClipboardSink;
use crate::errors::{Error, Result};
use es_backend::Backend;
use es_model::platform::ClipItem;
use es_store::Paths;
use es_store::temp::{self, TempKind};
use gpui::{App, Task};
use std::path::PathBuf;
use std::sync::Arc;

const CLIP_FILE: &str = "capture.png";

/// File managers and chat apps paste files, not image data, so an image without a file gets one.
pub fn with_file_items(
    mut items: Vec<ClipItem>,
    write_png: impl FnOnce(&[u8]) -> Result<PathBuf>,
) -> Result<Vec<ClipItem>> {
    if items.iter().any(|i| matches!(i, ClipItem::UriList(_))) {
        return Ok(items);
    }
    let png = items.iter().find_map(|i| match i {
        ClipItem::Png(png) => Some(png.clone()),
        _ => None,
    });
    if let Some(png) = png {
        let path = write_png(&png)?;
        items.push(ClipItem::UriList(vec![path.clone()]));
        items.push(ClipItem::GnomeCopiedFiles(vec![path]));
    }
    Ok(items)
}

pub struct BackendClipboardSink {
    backend: Arc<Backend>,
    paths: Paths,
}

impl BackendClipboardSink {
    pub fn new(backend: Arc<Backend>, paths: Paths) -> Self {
        BackendClipboardSink { backend, paths }
    }
}

impl ClipboardSink for BackendClipboardSink {
    fn copy(&self, items: Vec<ClipItem>, cx: &mut App) -> Task<Result<()>> {
        let backend = self.backend.clone();
        let paths = self.paths.clone();
        cx.background_executor().spawn(async move {
            let items = with_file_items(items, |png| {
                temp::create(&paths, TempKind::Clipboard, CLIP_FILE, png)
                    .map_err(|e| Error::Protocol(format!("clipboard file: {e}")))
            })?;
            backend
                .clipboard
                .set(items)
                .map_err(|e| Error::Protocol(format!("clipboard: {e}")))
        })
    }
}
