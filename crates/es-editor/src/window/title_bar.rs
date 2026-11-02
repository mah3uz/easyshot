use es_ui::{
    components::{ButtonKind, Capsule, EditorButton, IconButton},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, div,
    px,
};
use gpui_component::TitleBar;

use super::{EditorWindow, Job, Progress, TITLE_BAR_HEIGHT, busy, done};
use crate::strings::fl;

const HISTORY_BUTTON: gpui::Pixels = px(28.);

impl EditorWindow {
    pub(super) fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let history = Capsule::new("history", fl!("editor-history-a11y"))
            .child(
                IconButton::new("undo", IconName::Undo2, fl!("editor-undo"))
                    .size(HISTORY_BUTTON, px(16.))
                    .disabled(!self.can_undo(cx))
                    .on_click(cx.listener(|w, _, _, cx| w.undo(cx))),
            )
            .child(
                IconButton::new("redo", IconName::Redo2, fl!("editor-redo"))
                    .size(HISTORY_BUTTON, px(16.))
                    .disabled(!self.can_redo(cx))
                    .on_click(cx.listener(|w, _, _, cx| w.redo(cx))),
            );
        TitleBar::new()
            .h(TITLE_BAR_HEIGHT)
            .child(side().child(hold("history-hold").child(history)))
            .child(
                div()
                    .flex_none()
                    .min_w_0()
                    .max_w(px(520.))
                    .px(px(12.))
                    .truncate()
                    .typeset(t.type_scale.title)
                    .text_color(t.text)
                    .child(self.title(cx)),
            )
            .child(
                side()
                    .justify_end()
                    .pr(px(12.))
                    .child(if self.is_cropping(cx) {
                        hold("crop-hold").child(self.crop_controls(cx))
                    } else {
                        hold("actions-hold")
                            .gap(px(8.))
                            .child(self.save_control(cx))
                            .child(self.export_control(cx))
                    }),
            )
    }

    fn save_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        match self.progress(Job::Save) {
            Progress::Busy => busy("save", fl!("editor-saving-a11y"), false, &t),
            Progress::Done => done("save", fl!("editor-save-flash"), &t),
            Progress::Idle => EditorButton::new("save")
                .icon(IconName::Download)
                .label(fl!("editor-save"))
                .tooltip(fl!("editor-save-help-plain"))
                .disabled(!self.can_run(Job::Save, cx))
                .on_click(cx.listener(|w, _, _, cx| {
                    w.request(Job::Save, cx);
                }))
                .into_any_element(),
        }
    }

    fn export_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        match self.progress(Job::Export) {
            Progress::Busy => busy("export", fl!("editor-exporting-a11y"), true, &t),
            Progress::Idle | Progress::Done => EditorButton::new("export")
                .kind(ButtonKind::Prominent)
                .icon(IconName::CircleArrowDown)
                .label(fl!("editor-export"))
                .accessible_label(fl!("editor-export-a11y"))
                .tooltip(fl!("editor-export-help"))
                .disabled(!self.can_run(Job::Export, cx))
                .on_click(cx.listener(|w, _, _, cx| {
                    w.request(Job::Export, cx);
                }))
                .into_any_element(),
        }
    }
}

fn side() -> gpui::Div {
    div()
        .flex()
        .flex_1()
        .flex_basis(px(0.))
        .min_w_0()
        .items_center()
}

/// A pressed button must not start a window move, which the title bar begins on any press.
fn hold(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}
