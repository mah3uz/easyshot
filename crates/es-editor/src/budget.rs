use es_app::image_budget::ImageStores;
use gpui::{App, RenderImage};

/// One owner's claim on the shared image budget; every call does nothing when the app has none.
pub struct Lease {
    prefix: String,
}

impl Lease {
    pub fn new(owner: u64) -> Self {
        Self {
            prefix: format!("{owner}/"),
        }
    }

    fn key(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    /// Counts the image against the budget and marks it most recently used.
    pub fn hold(&self, cx: &App, name: &str, image: &RenderImage) {
        if let Some(stores) = cx.try_global::<ImageStores>() {
            let bytes = image.as_bytes(0).map_or(0, <[u8]>::len);
            stores.budget.insert(&self.key(name), bytes);
        }
    }

    pub fn release(&self, cx: &App, name: &str) {
        if let Some(stores) = cx.try_global::<ImageStores>() {
            stores.budget.remove(&self.key(name));
        }
    }

    pub fn release_group(&self, cx: &App, group: &str) {
        if let Some(stores) = cx.try_global::<ImageStores>() {
            stores.budget.remove_prefix(&self.key(group));
        }
    }

    /// Names in `group` the budget gave up since the last call; the owner drops those images.
    pub fn evicted(&self, cx: &App, group: &str) -> Vec<String> {
        let Some(stores) = cx.try_global::<ImageStores>() else {
            return Vec::new();
        };
        let prefix = self.prefix.len();
        stores
            .budget
            .take_evicted(&self.key(group))
            .into_iter()
            .map(|k| k[prefix..].to_owned())
            .collect()
    }
}
