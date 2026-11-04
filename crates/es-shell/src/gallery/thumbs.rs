use super::files::GalleryFiles;
use super::model::MediaKind;
use es_store::Paths;
use gpui::RenderImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub enum Thumb {
    Loading,
    Ready(Arc<RenderImage>),
    Unavailable,
}

struct Entry {
    thumb: Thumb,
    asked: u32,
    shown: u32,
}

#[derive(Default)]
pub struct Thumbs {
    by_file: HashMap<PathBuf, Entry>,
}

impl Thumbs {
    pub fn get(&self, file: &Path) -> Option<&Thumb> {
        self.by_file.get(file).map(|e| &e.thumb)
    }

    /// Marks the file as loading at `pixels` and says whether the caller should start the load; a smaller
    /// picture already shown stays until the larger one arrives.
    pub fn claim(&mut self, file: &Path, pixels: u32) -> bool {
        match self.by_file.get_mut(file) {
            None => {
                let entry = Entry {
                    thumb: Thumb::Loading,
                    asked: pixels,
                    shown: 0,
                };
                self.by_file.insert(file.to_path_buf(), entry);
            }
            Some(e) if e.asked >= pixels || matches!(e.thumb, Thumb::Unavailable) => return false,
            Some(e) => e.asked = pixels,
        }
        true
    }

    pub fn finish(&mut self, file: &Path, pixels: u32, image: Option<Arc<RenderImage>>) {
        let Some(entry) = self.by_file.get_mut(file) else {
            return;
        };
        if entry.asked != pixels {
            return;
        }
        match image {
            Some(image) => {
                entry.thumb = Thumb::Ready(image);
                entry.shown = pixels;
            }
            None if matches!(entry.thumb, Thumb::Ready(_)) => {}
            None => entry.thumb = Thumb::Unavailable,
        }
    }

    pub fn retain(&mut self, keep: impl Fn(&Path) -> bool) {
        self.by_file.retain(|file, _| keep(file));
    }

    pub fn ready(&self) -> usize {
        self.by_file
            .values()
            .filter(|e| matches!(e.thumb, Thumb::Ready(_)))
            .count()
    }

    /// Files whose shown picture was cached at `pixels` or more.
    pub fn ready_at(&self, pixels: u32) -> usize {
        self.by_file.values().filter(|e| e.shown >= pixels).count()
    }
}

/// Runs off the main thread: reads the disk cache, filling it first when it has no entry.
pub fn load(
    paths: &Paths,
    files: &dyn GalleryFiles,
    kind: MediaKind,
    file: &Path,
    pixels: u32,
) -> Option<Arc<RenderImage>> {
    let made = match kind {
        MediaKind::Screenshot => es_store::thumbs::ensure(paths, file, pixels),
        MediaKind::Video => {
            es_store::thumbs::ensure_video(paths, file, pixels, |f| files.video_frame(f, pixels))
        }
    };
    let thumb = made
        .inspect_err(|e| tracing::debug!(error = %e, "no thumbnail"))
        .ok()?;
    let mut rgba = es_store::decode::open_image(&thumb).ok()?.into_rgba8();
    // GPUI images are BGRA.
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([image::Frame::new(rgba)])))
}
