use super::EditorWindow;
use crate::strings::fl;
use gpui::{Context, EventEmitter, Task};
use std::path::PathBuf;

/// Share was pressed; whoever gave the window its popover host opens the popover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareRequested;

impl EventEmitter<ShareRequested> for EditorWindow {}

impl EditorWindow {
    /// Shows Share, with its popover in this window; without one the window has no Share.
    pub fn set_share(&mut self, host: es_app::share::WindowPopover, cx: &mut Context<Self>) {
        self.share = Some(host);
        cx.notify();
    }

    pub fn share(&mut self, cx: &mut Context<Self>) {
        if self.share.is_some() && self.session().is_some() && !self.is_cropping(cx) {
            cx.emit(ShareRequested);
        }
    }

    /// The image as edited, rendered as Export renders it, for sharing.
    pub fn share_file(&self, cx: &mut gpui::App) -> Task<std::result::Result<PathBuf, String>> {
        match self.session() {
            Some(session) => session.io.update(cx, |io, cx| io.render_for_share(cx)),
            None => Task::ready(Err(fl!("editor-error-unavailable"))),
        }
    }
}
