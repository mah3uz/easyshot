use super::{Recordings, agent::Ended, finish, saved_toast};
use es_app::deck::DeckItem;
use es_app::platform_ui::ClipboardSink;
use es_app::toast::{Toast, show_toast};
use es_model::platform::ClipItem;
use es_store::temp::{self, TempKind};
use gpui::App;
use std::rc::Rc;
use time::OffsetDateTime;

impl Recordings {
    pub(super) fn show_card(&self, item: DeckItem, save: bool, cx: &mut App) {
        let id = item.id;
        tracing::info!(file = %item.file.display(), "recording card shown");
        self.0.deps.deck.update(cx, |d, cx| d.show(item, cx));
        if save {
            self.save_card(id, cx);
        }
    }

    /// Remuxes the card's recording into the save folder; the card goes when that works.
    pub fn save_card(&self, id: uuid::Uuid, cx: &mut App) {
        let deps = &self.0.deps;
        let Some(video) = deps.deck.read(cx).get(id).map(|i| i.file.clone()) else {
            return;
        };
        let config = deps.settings.read(cx).config().clone();
        let (runner, paths) = (deps.runner.clone(), deps.paths.clone());
        let deck = deps.deck.clone();
        let me = self.clone();
        cx.spawn(async move |cx| {
            let saved = cx
                .background_executor()
                .spawn(async move {
                    let now = OffsetDateTime::now_utc();
                    finish::save_mp4(&video, &paths, &config, now, runner.as_ref())
                })
                .await;
            cx.update(|cx| match saved {
                Ok(path) => {
                    tracing::info!(file = %path.display(), "recording saved");
                    show_toast(saved_toast("toast-recording-saved", &path), cx);
                    deck.update(cx, |d, cx| d.saved(id, cx));
                    es_app::services::videos_changed(cx);
                    me.ended(&Ended::Saved { card: id, path }, cx);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "recording not saved");
                    show_toast(Toast::error("toast-recording-save-failed"), cx);
                    deck.update(cx, |d, cx| d.save_failed(id, cx));
                    me.ended(&Ended::SaveFailed { card: id }, cx);
                }
            });
        })
        .detach();
    }

    /// Puts the card's recording on the clipboard as an mp4 file; the card goes when that works.
    pub fn copy_card(&self, id: uuid::Uuid, clipboard: Rc<dyn ClipboardSink>, cx: &mut App) {
        let deps = &self.0.deps;
        let Some(item) = deps.deck.read(cx).get(id).cloned() else {
            return;
        };
        let (runner, paths) = (deps.runner.clone(), deps.paths.clone());
        let deck = deps.deck.clone();
        cx.spawn(async move |cx| {
            let file = cx
                .background_executor()
                .spawn(async move { copy_source(&item, &paths, runner.as_ref()) })
                .await;
            let copied = match file {
                Ok(file) => cx.update(|cx| {
                    clipboard.copy(
                        vec![
                            ClipItem::UriList(vec![file.clone()]),
                            ClipItem::GnomeCopiedFiles(vec![file]),
                        ],
                        cx,
                    )
                }),
                Err(e) => {
                    tracing::warn!(error = %e, "recording not prepared for the clipboard");
                    cx.update(|cx| show_toast(Toast::error("toast-copy-failed"), cx));
                    return;
                }
            };
            let result = copied.await;
            cx.update(|cx| match result {
                Ok(()) => deck.update(cx, |d, cx| d.copied(id, cx)),
                Err(e) => {
                    tracing::warn!(error = %e, "copy of a recording");
                    show_toast(Toast::error("toast-copy-failed"), cx);
                }
            });
        })
        .detach();
    }

    /// Cards for recordings that recovery finished at start-up.
    pub fn show_recovered(&self, found: &[es_record::recovery::Recovered], cx: &mut App) {
        self.0.deps.deck.update(cx, |deck, cx| {
            for r in found {
                deck.show(finish::recovered_card(r), cx);
            }
        });
    }
}

/// The saved mp4, or a fresh one in a temporary folder when the card was never saved.
fn copy_source(
    item: &DeckItem,
    paths: &es_store::Paths,
    runner: &dyn es_record::project::CommandRunner,
) -> Result<std::path::PathBuf, String> {
    if let Some(saved) = item.saved.as_ref().filter(|f| f.is_file()) {
        return Ok(saved.clone());
    }
    let dir = temp::make_dir(paths, TempKind::VideoCopy).map_err(|e| e.to_string())?;
    finish::remux(&item.file, dir.join(format!("{}.mp4", item.name)), runner)
}
