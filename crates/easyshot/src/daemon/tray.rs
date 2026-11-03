use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use es_app::deck::Deck;
use es_app::history::{History, HistoryEvent};
use es_app::settings_model::SettingsModel;
use es_desktop::RecordingTrayState;
use es_shell::tray::{self, TrayInput};
use gpui::{App, Entity, Subscription};

pub struct TrayFeed {
    show: Rc<dyn Fn(&TrayInput)>,
    has_pins: Rc<dyn Fn() -> bool>,
    history: Entity<History>,
    settings: Entity<SettingsModel>,
    videos: PathBuf,
    recording: Cell<Option<RecordingTrayState>>,
}

impl TrayFeed {
    pub fn new(
        show: Rc<dyn Fn(&TrayInput)>,
        has_pins: Rc<dyn Fn() -> bool>,
        history: Entity<History>,
        settings: Entity<SettingsModel>,
        videos: PathBuf,
    ) -> Rc<Self> {
        Rc::new(TrayFeed {
            show,
            has_pins,
            history,
            settings,
            videos,
            recording: Cell::new(None),
        })
    }

    pub fn input(&self, cx: &App) -> TrayInput {
        let screenshots = self
            .history
            .read(cx)
            .entries()
            .iter()
            .take(tray::RECENT_PER_KIND)
            .map(tray::Recent::from)
            .collect();
        TrayInput {
            shortcuts: tray::shortcuts(&self.settings.read(cx).config().global_shortcuts),
            screenshots,
            recordings: tray::recent_recordings(&self.videos),
            has_pins: (self.has_pins)(),
            recording: self.recording.get(),
            version: env!("EASYSHOT_VERSION").to_owned(),
            update: None,
        }
    }

    pub fn refresh(&self, cx: &App) {
        (self.show)(&self.input(cx));
    }

    pub fn set_recording(&self, state: Option<RecordingTrayState>, cx: &App) {
        self.recording.set(state);
        self.refresh(cx);
    }

    /// Re-shows the menu when history, settings or the deck change; pin changes call `refresh` themselves.
    pub fn follow(self: &Rc<Self>, deck: &Entity<Deck>, cx: &mut App) -> Vec<Subscription> {
        let again = |feed: &Rc<Self>| {
            let feed = feed.clone();
            move |cx: &mut App| feed.refresh(cx)
        };
        let (on_history, on_settings, on_deck) = (again(self), again(self), again(self));
        vec![
            cx.subscribe(&self.history, move |_, _: &HistoryEvent, cx| on_history(cx)),
            cx.observe(&self.settings, move |_, cx| on_settings(cx)),
            cx.observe(deck, move |_, cx| on_deck(cx)),
        ]
    }
}
