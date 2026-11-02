use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use es_store::Paths;
use gpui::{Context, RenderImage};
use image::{DynamicImage, Frame, imageops::FilterType};

/// Twice a built-in tile, so the thumbnail stays sharp on a 2× display.
const THUMB: (u32, u32) = (216, 160);

pub struct BuiltIn {
    pub id: &'static str,
    bytes: &'static [u8],
}

macro_rules! built_in {
    ($id:literal) => {
        BuiltIn {
            id: $id,
            bytes: include_bytes!(concat!("../../../../../assets/wallpapers/", $id, ".jpg")),
        }
    };
}

pub const BUILT_IN: [BuiltIn; 7] = [
    built_in!("wall-1"),
    built_in!("wall-2"),
    built_in!("wall-3"),
    built_in!("wall-4"),
    built_in!("wall-5"),
    built_in!("wall-6"),
    built_in!("wall-7"),
];

impl BuiltIn {
    fn file_name(&self) -> String {
        format!("{}.jpg", self.id)
    }

    /// Where the wallpaper lives once picked; documents and presets refer to it by this path.
    pub fn path(&self, paths: &Paths) -> PathBuf {
        paths.wallpapers_dir().join(self.file_name())
    }

    pub fn install(&self, paths: &Paths) -> es_store::Result<PathBuf> {
        es_store::wallpapers::install_built_in(paths, &self.file_name(), self.bytes)
    }
}

/// The encoded wallpaper a style points at; a built-in whose written copy was removed still draws.
pub fn wallpaper_bytes(path: &Path) -> Option<Cow<'static, [u8]>> {
    if let Some(bytes) = es_store::wallpapers::read(path) {
        return Some(Cow::Owned(bytes));
    }
    let in_wallpapers = path.parent()?.file_name()? == "wallpapers";
    let built_in = BUILT_IN
        .iter()
        .find(|b| path.file_name().is_some_and(|n| *n == *b.file_name()))?;
    in_wallpapers.then_some(Cow::Borrowed(built_in.bytes))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    BuiltIn(usize),
    File(PathBuf),
}

#[derive(Clone)]
pub enum Thumb {
    Loading,
    Ready(Arc<RenderImage>),
    Missing,
    Unreadable,
}

/// Wallpaper thumbnails, decoded off the main thread and kept for the life of the inspector.
#[derive(Default)]
pub struct WallpaperCache {
    thumbs: HashMap<Source, Thumb>,
}

impl WallpaperCache {
    pub fn get(&self, source: &Source) -> Thumb {
        self.thumbs.get(source).cloned().unwrap_or(Thumb::Loading)
    }

    pub fn is_ready(&self, source: &Source) -> bool {
        !matches!(self.get(source), Thumb::Loading)
    }

    /// Starts decoding the sources not seen yet; observers hear when each one lands.
    pub fn request(&mut self, sources: impl IntoIterator<Item = Source>, cx: &mut Context<Self>) {
        for source in sources {
            if self.thumbs.contains_key(&source) {
                continue;
            }
            self.thumbs.insert(source.clone(), Thumb::Loading);
            let job = source.clone();
            let decoding = cx
                .background_executor()
                .spawn(async move { thumbnail(&job) });
            cx.spawn(async move |this, cx| {
                let thumb = decoding.await;
                let _ = this.update(cx, |cache, cx| {
                    cache.thumbs.insert(source, thumb);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// A file chosen again after it went missing is decoded afresh.
    pub fn forget(&mut self, path: &Path) {
        self.thumbs.remove(&Source::File(path.to_path_buf()));
    }
}

fn thumbnail(source: &Source) -> Thumb {
    let decoded = match source {
        Source::BuiltIn(i) => BUILT_IN
            .get(*i)
            .and_then(|b| image::load_from_memory(b.bytes).ok()),
        Source::File(path) if !path.exists() => return Thumb::Missing,
        Source::File(path) => es_store::wallpapers::read(path)
            .and_then(|bytes| es_store::decode::image_from_memory(&bytes).ok()),
    };
    decoded
        .and_then(|image| render_image(&image))
        .map_or(Thumb::Unreadable, Thumb::Ready)
}

/// Cropped to the tile's shape so it fills the tile like the wallpaper fills the canvas.
fn render_image(image: &DynamicImage) -> Option<Arc<RenderImage>> {
    let mut bgra = image
        .resize_to_fill(THUMB.0, THUMB.1, FilterType::Triangle)
        .into_rgba8();
    for px in bgra.pixels_mut() {
        px.0.swap(0, 2);
    }
    (bgra.width() > 0).then(|| Arc::new(RenderImage::new([Frame::new(bgra)])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_or_a_missing_file_gives_no_wallpaper() {
        assert!(wallpaper_bytes(Path::new("/dev/zero")).is_none());
        assert!(wallpaper_bytes(Path::new("/nonexistent/private.png")).is_none());
        assert!(matches!(
            thumbnail(&Source::File("/dev/zero".into())),
            Thumb::Unreadable
        ));
    }
}
