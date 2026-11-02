use es_app::deck::{Deck, DeckAction, DeckEvent};
use es_app::dispatcher::{Action, ActionHandler};
use es_app::errors::Error;
use es_app::platform_ui::ClipboardSink;
use es_app::toast::{Toast, show_toast};
use es_app::windows::{WindowKind, Windows};
use es_model::platform::ClipItem;
use es_overlays::pin::Pins;
use es_store::Paths;
use gpui::{App, Entity, Subscription};
use std::rc::Rc;

fn not_wired(what: &str, cx: &mut App) -> Error {
    tracing::error!(what, "not wired into the daemon yet");
    show_toast(Toast::error("toast-unsupported"), cx);
    Error::Unsupported
}

pub fn register_windows(windows: &Windows) {
    for kind in [
        WindowKind::Settings,
        WindowKind::Gallery,
        WindowKind::Editor,
        WindowKind::CaptureBar,
    ] {
        windows.register(
            kind,
            Rc::new(move |_, cx| Err(not_wired(&format!("{kind:?} window"), cx))),
        );
    }
}

pub fn actions(deck: Entity<Deck>) -> ActionHandler {
    Rc::new(move |action, cx| {
        match action {
            Action::Quit => cx.quit(),
            Action::DeckSaveAll => deck.update(cx, |d, cx| d.save_all(cx)),
            Action::DeckCloseAll => deck.update(cx, |d, cx| d.clear_all(cx)),
            other => return Err(not_wired(&format!("{other:?}"), cx)),
        }
        Ok(())
    })
}

fn copy_card(file: &std::path::Path, clipboard: &Rc<dyn ClipboardSink>, cx: &mut App) {
    let png = match std::fs::read(file) {
        Ok(png) => png,
        Err(e) => {
            tracing::warn!(error = %e, "deck card unreadable");
            show_toast(Toast::error("toast-copy-failed"), cx);
            return;
        }
    };
    let copied = clipboard.copy(vec![ClipItem::Png(png.into())], cx);
    cx.spawn(async move |cx| {
        if let Err(e) = copied.await {
            tracing::warn!(error = %e, "copy from the deck");
            cx.update(|cx| show_toast(Toast::error("toast-copy-failed"), cx));
        }
    })
    .detach();
}

/// Card actions, and dropping the staged files of a card that was let go of.
pub fn deck_events(
    deck: &Entity<Deck>,
    paths: Paths,
    clipboard: Rc<dyn ClipboardSink>,
    pins: Rc<Pins>,
    cx: &mut App,
) -> Subscription {
    cx.subscribe(deck, move |deck, event: &DeckEvent, cx| match event {
        DeckEvent::Removed { item, why } => {
            if why.discards() && item.staged {
                let (paths, id) = (paths.clone(), item.id);
                cx.background_executor()
                    .spawn(async move {
                        if let Err(e) = es_store::Deck::new(&paths).remove(id) {
                            tracing::warn!(error = %e, "deck staging not removed");
                        }
                    })
                    .detach();
            }
        }
        DeckEvent::Requested { item, action } => match action {
            DeckAction::Copy => copy_card(&item.file, &clipboard, cx),
            DeckAction::Pin => {
                let pinned = es_overlays::deck::view::load_thumbnail(&item.file)
                    .ok_or_else(|| Error::NotFound(item.file.display().to_string()))
                    .and_then(|image| pins.pin(item.file.clone(), image, cx));
                if let Err(e) = pinned {
                    tracing::warn!(error = %e, "pin from the deck");
                    show_toast(Toast::error("toast-error-generic"), cx);
                }
            }
            DeckAction::Save => {
                let id = item.id;
                not_wired("deck Save", cx);
                deck.update(cx, |d, cx| d.save_failed(id, cx));
            }
            other => {
                not_wired(&format!("deck {other:?}"), cx);
            }
        },
    })
}
