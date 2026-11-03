use std::sync::Arc;

use gpui::Context;

use crate::session::preview;
use crate::window::EditorWindow;

impl EditorWindow {
    /// Starts the downscale off the main thread when the base needs one and has not been asked for yet.
    pub(super) fn refresh_preview(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.session().map(|s| s.doc.clone()) else {
            return;
        };
        let forced = self.force_low_res;
        let (revision, base) = {
            let d = doc.read(cx);
            (d.base_revision(), d.base().clone())
        };
        if self.preview_request == Some((revision, forced)) {
            return;
        }
        self.preview_request = Some((revision, forced));
        let wanted = preview::in_effect(base.width(), base.height(), forced);
        self.set_low_res(wanted, cx);
        if !wanted {
            self._preview = None;
            doc.update(cx, |d, cx| d.set_preview(revision, None, cx));
            return;
        }
        self._preview = Some(cx.spawn(async move |this, cx| {
            let image = cx
                .background_executor()
                .spawn(async move { preview::downscale(&base, forced).map(Arc::new) })
                .await;
            doc.update(cx, |d, cx| d.set_preview(revision, image, cx));
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }
}
