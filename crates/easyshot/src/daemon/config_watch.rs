use es_app::settings_model::SettingsModel;
use gpui::{App, Entity, Task};

/// The returned task must stay alive for as long as the watch should run.
pub(super) fn install(settings: &Entity<SettingsModel>, cx: &mut App) -> Option<Task<()>> {
    let (tx, rx) = async_channel::unbounded::<()>();
    let started = settings.update(cx, |s, _| {
        s.watch(move || {
            let _ = tx.try_send(());
        })
    });
    if let Err(e) = started {
        tracing::warn!(error = %e, "config.toml is not watched; edits apply on restart");
        return None;
    }
    let settings = settings.downgrade();
    Some(cx.spawn(async move |cx| {
        while rx.recv().await.is_ok() {
            if settings.update(cx, |s, cx| s.reload(cx)).is_err() {
                break;
            }
        }
    }))
}
