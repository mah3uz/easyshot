use es_app::deck::{Deck, DeckItem};
use es_app::history::History;
use es_app::toast::{CloseReason, IconName, Toast, show_toast};
use es_shell::gallery::GalleryFiles;
use es_shell::gallery::files::capture_files;
use es_store::Paths;
use gpui::{App, Entity};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
pub struct Deletion {
    pub deck: Entity<Deck>,
    pub history: Entity<History>,
    pub files: Arc<dyn GalleryFiles>,
    pub paths: Paths,
}

/// What the Trash takes with a card: the library files of a kept capture, the saved file of a staged one.
fn trashable(item: &DeckItem) -> Vec<PathBuf> {
    match (&item.saved, item.staged) {
        (Some(saved), true) => capture_files(saved),
        (None, true) => Vec::new(),
        (_, false) => capture_files(&item.file),
    }
}

/// Moves the card's files to the Trash and offers Undo; the card is already off the deck.
pub fn delete_card(d: &Deletion, item: DeckItem, cx: &mut App) {
    let files = trashable(&item);
    let entry = (!item.staged)
        .then(|| d.history.read(cx).by_file(&item.file).cloned())
        .flatten();
    let mut trashed = Vec::new();
    for file in &files {
        if let Err(error) = d.files.trash(file) {
            tracing::warn!(%error, "delete from the deck");
            if !trashed.is_empty()
                && let Err(error) = d.files.restore(&trashed)
            {
                tracing::warn!(%error, "partial delete not undone");
            }
            d.deck.update(cx, |deck, cx| deck.show(item, cx));
            show_toast(Toast::error("toast-delete-failed"), cx);
            return;
        }
        trashed.push(file.clone());
    }
    if let Some(entry) = &entry
        && let Err(error) = d.history.update(cx, |h, cx| h.remove(entry.id, cx))
    {
        tracing::warn!(%error, "history");
    }

    let open = Rc::new(Cell::new(true));
    let undo = {
        let (d, open, item, files) = (d.clone(), open.clone(), item.clone(), files.clone());
        move |cx: &mut App| {
            if !open.replace(false) {
                return;
            }
            if !files.is_empty()
                && let Err(error) = d.files.restore(&files)
            {
                tracing::warn!(%error, "undo delete");
                show_toast(Toast::error("toast-undo-failed"), cx);
                return;
            }
            if let Some(entry) = entry.clone()
                && let Err(error) = d.history.update(cx, |h, cx| h.add(entry, cx))
            {
                tracing::warn!(%error, "history");
            }
            d.deck.update(cx, |deck, cx| deck.show(item.clone(), cx));
        }
    };
    let title = if files.is_empty() {
        "toast-discarded"
    } else {
        "toast-deleted"
    };
    let (paths, id, staged) = (d.paths.clone(), item.id, item.staged);
    let cleanup = move |reason: CloseReason, cx: &mut App| {
        if reason == CloseReason::Undone || !open.replace(false) || !staged {
            return;
        }
        let paths = paths.clone();
        cx.background_executor()
            .spawn(async move {
                if let Err(e) = es_store::Deck::new(&paths).remove(id) {
                    tracing::warn!(error = %e, "deck staging not removed");
                }
            })
            .detach();
    };
    show_toast(
        Toast::new(title, IconName::Delete)
            .undoable(undo)
            .on_close(cleanup),
        cx,
    );
}
