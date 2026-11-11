use super::export::ExportJob;
use super::view::PlayerWindow;
use crate::capture::strings::fl;
use es_app::share::WindowPopover;
use es_ui::components::{ButtonKind, EditorButton};
use es_ui::icons::IconName;
use gpui::{
    Action, AnyElement, App, Context, EventEmitter, KeyBinding, KeyBindingContextPredicate, Window,
};
use std::path::PathBuf;
use std::rc::Rc;

pub const CONTEXT: &str = "PlayerWindow";

/// Share was pressed; whoever gave the viewer its popover host opens the popover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShareRequested;

impl EventEmitter<ShareRequested> for PlayerWindow {}

#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = easyshot_player, no_json)]
pub struct ShareVideo;

/// Binds Share Video to `key`, as GPUI spells it, keeping every other binding, so it can run again when the shortcuts change.
pub fn bind_share(key: Option<&str>, cx: &mut App) {
    let context = KeyBindingContextPredicate::parse(CONTEXT).ok().map(Rc::new);
    let share = key
        .and_then(|key| KeyBinding::load(key, Box::new(ShareVideo), context, false, None, cx).ok());
    let kept: Vec<KeyBinding> = cx
        .key_bindings()
        .borrow()
        .bindings()
        .filter(|b| !b.action().as_any().is::<ShareVideo>())
        .cloned()
        .collect();
    cx.clear_key_bindings();
    cx.bind_keys(kept.into_iter().chain(share));
}

impl PlayerWindow {
    /// Shows Share, with its popover in this window; without one the viewer has no Share.
    pub fn set_share(&mut self, host: WindowPopover, cx: &mut Context<Self>) {
        self.share = Some(host);
        cx.notify();
    }

    pub fn share(&mut self, cx: &mut Context<Self>) {
        if self.share.is_some() {
            cx.emit(ShareRequested);
        }
    }

    /// The clip as Export would make it now; sharing chooses its own output.
    pub fn share_job(&self) -> ExportJob {
        self.export_job(PathBuf::new())
    }

    pub(super) fn share_control(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let host = self.share.clone()?;
        let button = EditorButton::new("share")
            .kind(ButtonKind::Bordered)
            .icon(IconName::CloudUpload)
            .label(fl!("player-share"))
            .accessible_label(fl!("player-share-a11y"))
            .selected(host.is_open())
            .on_click(cx.listener(|p, _, _, cx| p.share(cx)));
        Some(host.around(button, gpui::Anchor::TopRight, window, cx))
    }
}
