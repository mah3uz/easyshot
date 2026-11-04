use super::Recordings;
use es_app::toast::{Toast, show_toast};
use es_overlays::record::ControlBar;
use es_record::AudioSource;
use gpui::{App, Entity};

impl Recordings {
    /// The bar shows the new state only once the recorder's stream has it.
    pub(super) fn mute(
        &self,
        source: AudioSource,
        muted: bool,
        bar: &Entity<ControlBar>,
        cx: &mut App,
    ) {
        let done = self
            .0
            .active
            .borrow_mut()
            .as_mut()
            .and_then(|s| s.handle.as_mut())
            .map(|h| h.set_muted(source, muted));
        match done {
            Some(Ok(())) => bar.update(cx, |b, cx| b.set_muted(source, muted, cx)),
            Some(Err(e)) => {
                tracing::warn!(error = %e, "mute");
                show_toast(Toast::error("toast-error-generic"), cx);
            }
            None => {}
        }
    }
}
