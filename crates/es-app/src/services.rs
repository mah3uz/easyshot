use crate::deck::Deck;
use crate::history::History;
use crate::image_budget::ImageStores;
use crate::jobs::{JobFinished, Jobs};
use crate::settings_model::SettingsModel;
use crate::share::Shares;
use crate::strings::loader;
use crate::toast::FluentArgs;
use crate::windows::Windows;
use es_backend::Backend;
use es_store::Paths;
use gpui::{Entity, Global};
use std::sync::Arc;

pub struct Strings;

impl Strings {
    pub fn get(&self, id: &str) -> String {
        loader().get(id)
    }

    pub fn get_args(&self, id: &str, args: &FluentArgs) -> String {
        loader().get_args(id, args.clone())
    }

    pub fn has(&self, id: &str) -> bool {
        loader().has(id)
    }
}

pub struct Stores {
    pub paths: Paths,
}

pub struct Services {
    pub backend: Arc<Backend>,
    pub settings: Entity<SettingsModel>,
    pub history: Entity<History>,
    pub shares: Entity<Shares>,
    pub deck: Entity<Deck>,
    pub jobs: Jobs,
    pub windows: Windows,
    pub images: ImageStores,
    pub strings: Strings,
    pub stores: Stores,
}

impl Global for Services {}

impl Services {
    /// Also returns the receiver of finished jobs, for the D-Bus layer to forward.
    pub fn new(
        backend: Arc<Backend>,
        settings: Entity<SettingsModel>,
        history: Entity<History>,
        shares: Entity<Shares>,
        deck: Entity<Deck>,
        windows: Windows,
        paths: Paths,
    ) -> (Self, async_channel::Receiver<JobFinished>) {
        let (jobs, finished) = Jobs::new();
        let services = Self {
            backend,
            settings,
            history,
            shares,
            deck,
            jobs,
            windows,
            images: ImageStores::default(),
            strings: Strings,
            stores: Stores { paths },
        };
        (services, finished)
    }
}

/// Tells the gallery and the tray that a video was saved, exported or deleted; a no-op before services exist.
pub fn videos_changed(cx: &mut gpui::App) {
    let Some(history) = cx.try_global::<Services>().map(|s| s.history.clone()) else {
        return;
    };
    history.update(cx, |h, cx| h.videos_changed(cx));
}
