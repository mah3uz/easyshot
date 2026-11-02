use es_store::Paths;
use gpui::RenderImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Twice the grid's 64 px art, so a scaled display stays sharp.
pub const THUMB_SIZE: u32 = 128;

#[derive(Clone)]
pub enum Thumb {
    Loading,
    Ready(Arc<RenderImage>),
    Unavailable,
}

#[derive(Default)]
pub struct Thumbs {
    by_file: HashMap<PathBuf, Thumb>,
}

impl Thumbs {
    pub fn get(&self, file: &Path) -> Option<&Thumb> {
        self.by_file.get(file)
    }

    /// Marks the file as loading and says whether the caller should start the load.
    pub fn claim(&mut self, file: &Path) -> bool {
        if self.by_file.contains_key(file) {
            return false;
        }
        self.by_file.insert(file.to_path_buf(), Thumb::Loading);
        true
    }

    pub fn finish(&mut self, file: PathBuf, image: Option<Arc<RenderImage>>) {
        let thumb = image.map_or(Thumb::Unavailable, Thumb::Ready);
        self.by_file.insert(file, thumb);
    }

    pub fn retain(&mut self, keep: impl Fn(&Path) -> bool) {
        self.by_file.retain(|file, _| keep(file));
    }

    pub fn ready(&self) -> usize {
        self.by_file
            .values()
            .filter(|t| matches!(t, Thumb::Ready(_)))
            .count()
    }
}

/// Runs off the main thread: reads the disk cache, filling it first when it has no entry.
pub fn load(paths: &Paths, file: &Path) -> Option<Arc<RenderImage>> {
    let thumb = es_store::thumbs::ensure(paths, file, THUMB_SIZE)
        .inspect_err(|e| tracing::debug!(error = %e, "no thumbnail"))
        .ok()?;
    let mut rgba = image::open(thumb).ok()?.into_rgba8();
    // GPUI images are BGRA.
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([image::Frame::new(rgba)])))
}
