use super::coming_soon::{Feature, coming_soon};
use super::deck_delete::{Deletion, delete_card};
use super::recording::Recordings;
use super::windows::Wiring;
use es_app::agent_access::AgentAccessHandle;
use es_app::deck::{Deck, DeckAction, DeckEvent, DeckItem, ItemStatus};
use es_app::deck_save;
use es_app::dispatcher::{Action, ActionHandler};
use es_app::errors::Error;
use es_app::history::History;
use es_app::pipeline::THUMB_SIZE;
use es_app::platform_ui::ClipboardSink;
use es_app::toast::{IconName, Toast, show_toast};
use es_app::windows::{WindowKey, WindowKind};
use es_model::HistoryEntry;
use es_model::platform::ClipItem;
use es_overlays::pin::Pins;
use es_store::Paths;
use es_store::temp::{self, TempKind};
use gpui::{App, Entity, Subscription};
use gpui::{ClipboardEntry, ImageFormat};
use std::rc::Rc;

pub struct ActionDeps {
    pub deck: Entity<Deck>,
    pub history: Entity<History>,
    pub wiring: Rc<Wiring>,
    pub pins: Rc<Pins>,
    pub paths: Paths,
    pub recordings: Recordings,
}

fn latest(history: &Entity<History>, cx: &App) -> Option<HistoryEntry> {
    history
        .read(cx)
        .entries()
        .iter()
        .max_by_key(|e| e.created_at)
        .cloned()
}

fn pin_file(pins: &Pins, file: &std::path::Path, cx: &mut App) -> Result<(), Error> {
    let image = es_overlays::deck::view::load_thumbnail(file)
        .ok_or_else(|| Error::NotFound(file.display().to_string()))?;
    pins.pin(file.to_owned(), image, cx).map(|_| ())
}

fn restore_last(d: &ActionDeps, cx: &mut App) -> Result<(), Error> {
    let entry = latest(&d.history, cx).ok_or_else(|| Error::NotFound("a capture".into()))?;
    let thumb = es_store::thumbs::ensure(&d.paths, &entry.file, THUMB_SIZE)
        .map_err(|e| Error::Store(e.to_string()))?;
    let item = DeckItem {
        id: entry.id,
        name: entry.name,
        file: entry.file,
        thumb,
        kind: entry.kind,
        video: false,
        staged: false,
        status: ItemStatus::Idle,
        auto_dismiss: false,
        saved: entry.export_path,
    };
    d.deck.update(cx, |deck, cx| deck.show(item, cx));
    Ok(())
}

fn edit_clipboard(d: &ActionDeps, cx: &mut App) -> Result<(), Error> {
    let image = cx
        .read_from_clipboard()
        .and_then(|item| {
            item.entries().iter().find_map(|e| match e {
                ClipboardEntry::Image(image) => Some(image.clone()),
                _ => None,
            })
        })
        .ok_or_else(|| Error::NotFound("an image on the clipboard".into()))?;
    let ext = match image.format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
        _ => return Err(Error::Unsupported),
    };
    let file = temp::create(
        &d.paths,
        TempKind::Clipboard,
        &format!("clipboard.{ext}"),
        &image.bytes,
    )
    .map_err(|e| Error::Store(e.to_string()))?;
    d.wiring.open_editor_for(&file, cx)
}

pub fn revoke_agent(cx: &mut App) {
    let access = cx.global::<AgentAccessHandle>().0.clone();
    access.update(cx, |access, cx| access.revoke(cx));
}

fn open_video(d: &ActionDeps, file: std::path::PathBuf, cx: &mut App) -> Result<(), Error> {
    #[cfg(feature = "player")]
    return d.recordings.trim(file, cx);
    #[cfg(not(feature = "player"))]
    {
        let _ = (d, file, cx);
        Err(Error::Unsupported)
    }
}

pub fn actions(d: ActionDeps) -> ActionHandler {
    Rc::new(move |action, cx| {
        match action {
            Action::Quit => d.wiring.quit(cx),
            Action::DeckSaveAll => d.deck.update(cx, |d, cx| d.save_all(cx)),
            Action::DeckCloseAll => d.deck.update(cx, |d, cx| d.clear_all(cx)),
            Action::DeckToggle => d.deck.update(cx, |d, cx| d.toggle_hidden(cx)),
            Action::RecordStop => d.recordings.stop(cx)?,
            Action::RecordPause => d.recordings.toggle_pause(cx)?,
            Action::RecordRestart => d.recordings.restart(cx)?,
            Action::RecordDiscard => d.recordings.discard(cx)?,
            Action::ShowHome(tab) => d.wiring.show_home(tab, cx),
            Action::OpenVideo(file) => open_video(&d, file, cx)?,
            Action::UnpinAll => d.pins.unpin_all(cx),
            Action::RevokeAgent => revoke_agent(cx),
            Action::PinLast => {
                let entry =
                    latest(&d.history, cx).ok_or_else(|| Error::NotFound("a capture".into()))?;
                pin_file(&d.pins, &entry.file, cx)?;
            }
            Action::RestoreLast => restore_last(&d, cx)?,
            Action::EditClipboard => edit_clipboard(&d, cx)?,
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
    history: &Entity<History>,
    wiring: &Rc<Wiring>,
    pins: &Rc<Pins>,
    paths: &Paths,
    clipboard: Rc<dyn ClipboardSink>,
    cx: &mut App,
) -> Subscription {
    let (paths, pins, wiring, history) =
        (paths.clone(), pins.clone(), wiring.clone(), history.clone());
    let deletion = Deletion {
        deck: deck.clone(),
        history: history.clone(),
        files: wiring.files.clone(),
        paths: paths.clone(),
    };
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
        DeckEvent::Requested { item, .. } if item.video => {}
        DeckEvent::Requested { item, action } => match action {
            DeckAction::Copy => copy_card(&item.file, &clipboard, cx),
            DeckAction::Pin => {
                let pinned =
                    deck_save::pinnable(&paths, item).and_then(|file| pin_file(&pins, &file, cx));
                if let Err(e) = pinned {
                    tracing::warn!(error = %e, "pin from the deck");
                    show_toast(Toast::error("toast-error-generic"), cx);
                }
            }
            DeckAction::Edit => {
                let opened = deck_save::editable(&paths, item)
                    .and_then(|file| wiring.open_editor_for(&file, cx));
                if let Err(e) = opened {
                    tracing::warn!(error = %e, "edit from the deck");
                    show_toast(Toast::error("toast-error-generic"), cx);
                }
            }
            DeckAction::Save => save_card(&deck, item.clone(), &paths, &wiring, &history, cx),
            DeckAction::ShowInFolder => {
                if let Some(file) = item.saved.clone() {
                    let request = wiring.files.show_in_folder(file);
                    cx.spawn(async move |_| {
                        if let Err(error) = request.await {
                            tracing::warn!(%error, "show in folder from the deck");
                        }
                    })
                    .detach();
                }
            }
            DeckAction::Preview => {
                if let Err(e) =
                    wiring
                        .windows
                        .open(WindowKind::Preview, WindowKey::Path(item.file.clone()), cx)
                {
                    tracing::warn!(error = %e, "preview from the deck");
                    show_toast(Toast::error("toast-preview-failed"), cx);
                }
            }
            DeckAction::Delete => delete_card(&deletion, item.clone(), cx),
            DeckAction::Share => coming_soon(Feature::CloudShare, cx),
            DeckAction::Dismiss => {}
        },
    })
}

fn save_card(
    deck: &Entity<Deck>,
    item: DeckItem,
    paths: &Paths,
    wiring: &Rc<Wiring>,
    history: &Entity<History>,
    cx: &mut App,
) {
    let config = wiring.settings.read(cx).config().clone();
    let (paths, deck, history, id) = (paths.clone(), deck.clone(), history.clone(), item.id);
    let work = cx
        .background_executor()
        .spawn(async move { deck_save::save_card(&paths, &config, &item) });
    cx.spawn(async move |cx| {
        let result = work.await;
        cx.update(|cx| match result {
            Ok(saved) => {
                if let Some(entry) = saved.entry
                    && let Err(e) = history.update(cx, |h, cx| h.add(entry, cx))
                {
                    tracing::warn!(error = %e, "history");
                }
                deck.update(cx, |d, cx| d.saved(id, cx));
                let toast = Toast::new("toast-saved", IconName::Check)
                    .arg("path", saved.file.display().to_string());
                show_toast(es_app::pipeline::capture_notice(toast, &saved.file), cx);
            }
            Err(e) => {
                tracing::warn!(error = %e, "save from the deck");
                deck.update(cx, |d, cx| d.save_failed(id, cx));
                show_toast(Toast::error("toast-save-failed"), cx);
            }
        });
    })
    .detach();
}

/// Save and Edit on a recording's card; the other card actions do not apply to a video.
pub fn video_cards(d: &ActionDeps, cx: &mut App) -> Subscription {
    let recordings = d.recordings.clone();
    cx.subscribe(&d.deck, move |_, event: &DeckEvent, cx| {
        let DeckEvent::Requested { item, action } = event else {
            return;
        };
        if !item.video {
            return;
        }
        match action {
            DeckAction::Save => recordings.save_card(item.id, cx),
            DeckAction::Edit => {
                #[cfg(feature = "player")]
                let opened = recordings.trim(item.file.clone(), cx);
                #[cfg(not(feature = "player"))]
                let opened: Result<(), Error> = Err(Error::Unsupported);
                if let Err(e) = opened {
                    tracing::warn!(error = %e, "trim viewer");
                    show_toast(Toast::error("toast-unsupported"), cx);
                }
            }
            _ => {}
        }
    })
}
