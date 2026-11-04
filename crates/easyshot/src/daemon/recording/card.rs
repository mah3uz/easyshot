use super::{Recordings, agent::Ended, finish, saved_toast};
use es_app::deck::DeckItem;
use es_app::toast::{Toast, show_toast};
use gpui::App;
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

    /// Cards for recordings that recovery finished at start-up.
    pub fn show_recovered(&self, found: &[es_record::recovery::Recovered], cx: &mut App) {
        self.0.deps.deck.update(cx, |deck, cx| {
            for r in found {
                deck.show(finish::recovered_card(r), cx);
            }
        });
    }
}
