use super::wallpaper::shrink;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;
use tiny_skia::Pixmap;

pub const MAX_IMAGES: usize = 4;
pub const MAX_BYTES: usize = 192 * 1024 * 1024;

struct Entry {
    path: PathBuf,
    stamp: Option<SystemTime>,
    source: Arc<Pixmap>,
    /// Smaller copies in steps of √2, keyed by how many steps below the source they are.
    variants: Vec<(u32, Arc<Pixmap>)>,
    used: u64,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.variants
            .iter()
            .map(|(_, p)| p.data().len())
            .sum::<usize>()
            + self.source.data().len()
    }
}

#[derive(Default)]
struct Inner {
    entries: Vec<Entry>,
    clock: u64,
}

/// Decoded wallpapers and their reduced copies, so a look change or a zoom step does not decode the file again.
pub struct WallpaperCache {
    inner: Mutex<Inner>,
    max_images: usize,
    max_bytes: usize,
}

impl Default for WallpaperCache {
    fn default() -> Self {
        Self::new(MAX_IMAGES, MAX_BYTES)
    }
}

impl WallpaperCache {
    pub fn new(max_images: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::default(),
            max_images,
            max_bytes,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bytes(&self) -> usize {
        self.lock().entries.iter().map(Entry::bytes).sum()
    }

    /// The wallpaper at `path` in a size close to covering `cover` pixels, never smaller. `load` decodes the file on a miss;
    /// a file changed since it was cached is loaded again.
    pub fn get(
        &self,
        path: &Path,
        cover: (u32, u32),
        load: impl FnOnce() -> Option<Pixmap>,
    ) -> Option<Arc<Pixmap>> {
        let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if let Some(found) = self.lookup(path, stamp, cover) {
            return Some(found);
        }
        let source = Arc::new(load()?);
        let mut inner = self.lock();
        inner.entries.retain(|e| e.path != path);
        inner.clock += 1;
        let used = inner.clock;
        inner.entries.push(Entry {
            path: path.to_path_buf(),
            stamp,
            source,
            variants: Vec::new(),
            used,
        });
        drop(inner);
        self.lookup(path, stamp, cover)
    }

    fn lookup(
        &self,
        path: &Path,
        stamp: Option<SystemTime>,
        cover: (u32, u32),
    ) -> Option<Arc<Pixmap>> {
        let mut inner = self.lock();
        inner.clock += 1;
        let clock = inner.clock;
        let at = inner
            .entries
            .iter()
            .position(|e| e.path == path && e.stamp == stamp)?;
        let entry = &mut inner.entries[at];
        entry.used = clock;
        let step = steps_below(&entry.source, cover);
        let found = if step == 0 {
            entry.source.clone()
        } else if let Some((_, v)) = entry.variants.iter().find(|(s, _)| *s == step) {
            v.clone()
        } else {
            let variant = Arc::new(shrink(&entry.source, 2f64.powf(-f64::from(step) / 2.0))?);
            entry.variants.push((step, variant.clone()));
            variant
        };
        self.evict(&mut inner);
        Some(found)
    }

    fn evict(&self, inner: &mut Inner) {
        while inner.entries.len() > 1
            && (inner.entries.len() > self.max_images
                || inner.entries.iter().map(Entry::bytes).sum::<usize>() > self.max_bytes)
        {
            let Some(oldest) = inner
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.used)
                .map(|(i, _)| i)
            else {
                return;
            };
            inner.entries.swap_remove(oldest);
        }
    }
}

/// How many √2 steps the source can shrink and still cover `cover`.
fn steps_below(source: &Pixmap, cover: (u32, u32)) -> u32 {
    let need = (f64::from(cover.0) / f64::from(source.width()))
        .max(f64::from(cover.1) / f64::from(source.height()));
    if need >= 1.0 || need <= 0.0 {
        return 0;
    }
    (-2.0 * need.log2()).floor() as u32
}
