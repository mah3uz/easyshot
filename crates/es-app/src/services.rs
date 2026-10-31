use crate::deck::Deck;
use crate::history::History;
use crate::image_budget::ImageBudget;
use crate::jobs::Jobs;
use crate::settings_model::SettingsModel;
use crate::windows::Windows;
use es_backend::Backend;
use gpui::{Entity, Global};
use std::sync::Arc;

pub struct Strings {}

pub struct Stores {}

pub struct Services {
    pub backend: Arc<Backend>,
    pub settings: Entity<SettingsModel>,
    pub history: Entity<History>,
    pub deck: Entity<Deck>,
    pub jobs: Jobs,
    pub windows: Windows,
    pub images: ImageBudget,
    pub strings: Strings,
    pub stores: Stores,
}

impl Global for Services {}
