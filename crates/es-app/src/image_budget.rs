use es_render::beautify::wallpaper_cache::WallpaperCache;
use gpui::Global;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub const DEFAULT_CAP_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct Lru {
    cap: usize,
    used: usize,
    clock: u64,
    items: HashMap<String, (usize, u64)>,
}

impl Lru {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            used: 0,
            clock: 0,
            items: HashMap::new(),
        }
    }

    pub fn used(&self) -> usize {
        self.used
    }

    pub fn contains(&self, key: &str) -> bool {
        self.items.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.items.keys()
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Returns the keys evicted to fit, least recently used first; the inserted key itself is never evicted.
    pub fn insert(&mut self, key: &str, bytes: usize) -> Vec<String> {
        let at = self.tick();
        if let Some((old, _)) = self.items.insert(key.to_owned(), (bytes, at)) {
            self.used -= old;
        }
        self.used += bytes;
        let mut evicted = Vec::new();
        while self.used > self.cap {
            let Some(oldest) = self
                .items
                .iter()
                .filter(|(k, _)| k.as_str() != key)
                .min_by_key(|(_, (_, at))| *at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.remove(&oldest);
            evicted.push(oldest);
        }
        evicted
    }

    pub fn touch(&mut self, key: &str) {
        let at = self.tick();
        if let Some(item) = self.items.get_mut(key) {
            item.1 = at;
        }
    }

    pub fn remove(&mut self, key: &str) -> bool {
        match self.items.remove(key) {
            Some((bytes, _)) => {
                self.used -= bytes;
                true
            }
            None => false,
        }
    }
}

/// Both are shared by every window; a window finds them with `try_global`.
#[derive(Clone, Default)]
pub struct ImageStores {
    pub budget: Arc<ImageBudget>,
    pub wallpapers: Arc<WallpaperCache>,
}

impl Global for ImageStores {}

pub struct ImageBudget {
    lru: Mutex<Lru>,
    /// Keys evicted and not yet collected by their owner; a window drops its own images when it next paints.
    evicted: Mutex<Vec<String>>,
}

impl ImageBudget {
    pub fn new(cap: usize) -> Self {
        Self {
            lru: Mutex::new(Lru::new(cap)),
            evicted: Mutex::new(Vec::new()),
        }
    }

    fn lru(&self) -> MutexGuard<'_, Lru> {
        self.lru.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn insert(&self, key: &str, bytes: usize) -> Vec<String> {
        let evicted = self.lru().insert(key, bytes);
        let mut pending = self.pending();
        // An image held again after its eviction is in use, so its owner must not drop it.
        pending.retain(|k| k != key);
        pending.extend(evicted.iter().cloned());
        evicted
    }

    fn pending(&self) -> MutexGuard<'_, Vec<String>> {
        self.evicted.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Removes and returns the evicted keys that start with `prefix`.
    pub fn take_evicted(&self, prefix: &str) -> Vec<String> {
        let mut pending = self.pending();
        if pending.is_empty() {
            return Vec::new();
        }
        let (mine, rest) = std::mem::take(&mut *pending)
            .into_iter()
            .partition(|k| k.starts_with(prefix));
        *pending = rest;
        mine
    }

    pub fn remove_prefix(&self, prefix: &str) {
        let mut lru = self.lru();
        let keys: Vec<String> = lru
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for key in keys {
            lru.remove(&key);
        }
    }

    pub fn touch(&self, key: &str) {
        self.lru().touch(key);
    }

    pub fn remove(&self, key: &str) -> bool {
        self.lru().remove(key)
    }

    pub fn used(&self) -> usize {
        self.lru().used()
    }
}

impl Default for ImageBudget {
    fn default() -> Self {
        Self::new(DEFAULT_CAP_BYTES)
    }
}
