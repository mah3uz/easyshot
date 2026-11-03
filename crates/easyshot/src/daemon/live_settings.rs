use es_app::deck::{Deck, DeckPolicy};
use es_app::history::History;
use es_app::settings_model::{SectionMask, SettingsEvent, SettingsModel};
use es_app::toast::{Toast, show_toast};
use es_overlays::deck::view::{DeckSettings, DeckSurface};
use gpui::{App, Entity, Subscription};
use std::rc::Rc;

pub(super) fn install(
    settings: &Entity<SettingsModel>,
    deck: &Entity<Deck>,
    deck_surface: &Rc<DeckSurface>,
    history: &Entity<History>,
    cx: &mut App,
) -> Subscription {
    let (deck, deck_surface, history) = (deck.clone(), deck_surface.clone(), history.clone());
    cx.subscribe(settings, move |settings, event, cx| match event {
        SettingsEvent::Changed(mask) => {
            let cfg = settings.read(cx).config().clone();
            if mask.intersects(SectionMask::OVERLAY | SectionMask::CAPTURE) {
                deck.update(cx, |d, cx| {
                    d.set_policy(DeckPolicy::from_settings(&cfg.overlay, &cfg.capture), cx)
                });
            }
            if mask.contains(SectionMask::OVERLAY) {
                deck_surface.set_settings(DeckSettings::from_settings(&cfg.overlay), cx);
            }
            if mask.contains(SectionMask::GENERAL) {
                let limit = cfg.general.history_limit as usize;
                let result = history.update(cx, |h, cx| h.set_limit(limit, cx));
                if let Err(e) = result {
                    tracing::warn!(error = %e, "history limit not applied");
                }
            }
        }
        SettingsEvent::Invalid { line, col, message } => show_toast(
            Toast::error("toast-settings-invalid")
                .arg("line", line.to_string())
                .arg("col", col.to_string())
                .arg("message", message.clone()),
            cx,
        ),
        SettingsEvent::SaveFailed(error) => show_toast(
            Toast::error("toast-settings-save-failed").arg("error", error.clone()),
            cx,
        ),
    })
}
