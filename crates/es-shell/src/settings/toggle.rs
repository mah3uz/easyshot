use super::SettingsWindow;
use super::widgets::{row, switch};
use crate::strings::fl;
use es_model::Config;
use gpui::{AnyElement, Context, SharedString};
use gpui_component::Disableable as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    ShowBarAtLaunch,
    EditorsFullscreen,
    LowResPreview,
    AutoSave,
    CopyAfterSave,
    PlaySound,
    CaptureOnRelease,
    Preselect,
    OpenEditor,
    KeepPreviews,
    AlwaysShowActions,
    SystemAudio,
    Keystrokes,
    RecordingOpenEditor,
    RecordingSave,
    WholeScreenBar,
    CardCopy,
    CardSave,
    CardEdit,
    CardPin,
    CardShare,
    CardPreview,
    CardDelete,
}

impl Toggle {
    pub fn get(self, c: &Config) -> bool {
        *Self::field(self, &mut c.clone())
    }

    pub(super) fn field(self, c: &mut Config) -> &mut bool {
        match self {
            Toggle::ShowBarAtLaunch => &mut c.general.show_capture_bar_at_launch,
            Toggle::EditorsFullscreen => &mut c.general.editor_opens_fullscreen,
            Toggle::LowResPreview => &mut c.general.low_res_preview,
            Toggle::AutoSave => &mut c.general.auto_save,
            Toggle::CopyAfterSave => &mut c.general.copy_after_save,
            Toggle::PlaySound => &mut c.general.play_sound,
            Toggle::CaptureOnRelease => &mut c.capture.capture_on_release,
            Toggle::Preselect => &mut c.capture.preselect_previous,
            Toggle::OpenEditor => &mut c.capture.open_editor_after_capture,
            Toggle::KeepPreviews => &mut c.capture.keep_in_deck_until_saved,
            Toggle::AlwaysShowActions => &mut c.overlay.always_show_actions,
            Toggle::SystemAudio => &mut c.recording.system_audio,
            Toggle::Keystrokes => &mut c.recording.keystrokes,
            Toggle::RecordingOpenEditor => &mut c.recording.open_editor_after,
            Toggle::RecordingSave => &mut c.recording.save,
            Toggle::WholeScreenBar => &mut c.recording.whole_screen_bar,
            Toggle::CardCopy => &mut c.overlay.actions.copy,
            Toggle::CardSave => &mut c.overlay.actions.save,
            Toggle::CardEdit => &mut c.overlay.actions.edit,
            Toggle::CardPin => &mut c.overlay.actions.pin,
            Toggle::CardShare => &mut c.overlay.actions.share,
            Toggle::CardPreview => &mut c.overlay.actions.preview,
            Toggle::CardDelete => &mut c.overlay.actions.delete,
        }
    }

    pub(super) fn id(self) -> SharedString {
        SharedString::from(format!("toggle-{self:?}"))
    }
}

impl SettingsWindow {
    pub(super) fn card_action_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let actions = self.config(cx).overlay.actions;
        let last = [
            actions.copy,
            actions.save,
            actions.edit,
            actions.pin,
            actions.share,
            actions.preview,
            actions.delete,
        ]
        .iter()
        .filter(|on| **on)
        .count()
            == 1;
        [
            (
                Toggle::CardCopy,
                fl!("overlay-action-copy"),
                fl!("overlay-action-copy-help"),
            ),
            (
                Toggle::CardSave,
                fl!("overlay-action-save"),
                fl!("overlay-action-save-help"),
            ),
            (
                Toggle::CardEdit,
                fl!("overlay-action-edit"),
                fl!("overlay-action-edit-help"),
            ),
            (
                Toggle::CardPin,
                fl!("overlay-action-pin"),
                fl!("overlay-action-pin-help"),
            ),
            (
                Toggle::CardShare,
                fl!("overlay-action-share"),
                fl!("overlay-action-share-help"),
            ),
            (
                Toggle::CardPreview,
                fl!("overlay-action-preview"),
                fl!("overlay-action-preview-help"),
            ),
            (
                Toggle::CardDelete,
                fl!("overlay-action-delete"),
                fl!("overlay-action-delete-help"),
            ),
        ]
        .into_iter()
        .map(|(toggle, title, help)| {
            let on = toggle.get(&self.config(cx));
            let control = switch(
                toggle.id(),
                title.clone(),
                on,
                cx.listener(move |this, on: &bool, _, cx| this.set_toggle(toggle, *on, cx)),
            )
            .tooltip(help)
            .disabled(on && last);
            row(title, None, control, cx)
        })
        .collect()
    }
}
