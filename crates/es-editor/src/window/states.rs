use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, DotGridBackground, EditorButton},
    icons::IconName,
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{Icon, Sizable, spinner::Spinner};

use crate::strings::fl;

fn centred() -> gpui::Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
}

pub(super) fn loading(t: &Tokens) -> AnyElement {
    div()
        .size_full()
        .child(DotGridBackground::new())
        .child(
            centred().child(
                div()
                    .id("loading")
                    .accessible(Role::ProgressIndicator, fl!("editor-loading-a11y"))
                    .child(Spinner::new().large().color(t.text_secondary)),
            ),
        )
        .into_any_element()
}

pub(super) fn failed(t: &Tokens) -> AnyElement {
    let message = fl!("editor-error-load");
    centred()
        .gap(px(14.))
        .p(px(32.))
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(px(34.))
                .text_color(t.text_secondary),
        )
        .child(
            div()
                .id("load-error")
                .max_w(px(360.))
                .text_center()
                .text_size(px(13.))
                .text_color(t.text_secondary)
                .accessible(Role::Alert, message.clone())
                .child(message),
        )
        .child(
            EditorButton::new("close")
                .kind(ButtonKind::Bordered)
                .label(fl!("editor-close"))
                .on_click(|_, window, _| window.remove_window()),
        )
        .into_any_element()
}

pub(super) fn error_strip(error: SharedString, t: &Tokens) -> AnyElement {
    div()
        .id("error-strip")
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .px(px(18.))
        .py(px(10.))
        .bg(t.window.opacity(0.92))
        .border_t(t.metrics.hairline)
        .border_color(t.border)
        .text_color(t.destructive)
        .typeset(t.type_scale.label)
        .accessible(Role::Alert, error.clone())
        .child(error)
        .into_any_element()
}

/// The same footprint as an editor button, for the busy and done looks that replace one.
fn button_shell(id: &'static str, t: &Tokens) -> gpui::Stateful<gpui::Div> {
    let m = t.metrics;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .h(m.button_height)
        .min_w(m.button_height)
        .px(m.button_padding_x)
        .rounded(m.button_radius)
        .typeset(t.type_scale.tool_button)
}

pub(crate) fn busy(id: &'static str, label: String, prominent: bool, t: &Tokens) -> AnyElement {
    let colour = if prominent { t.on_accent } else { t.text };
    button_shell(id, t)
        .when(prominent, |d| d.bg(t.accent.opacity(0.8)))
        .accessible(Role::ProgressIndicator, label)
        .child(Spinner::new().small().color(colour))
        .into_any_element()
}

pub(crate) fn done(id: &'static str, label: String, t: &Tokens) -> AnyElement {
    button_shell(id, t)
        .text_color(t.success)
        .accessible(Role::Status, label.clone())
        .child(Icon::new(IconName::CircleCheck).size(px(16.)).flex_none())
        .child(label)
        .into_any_element()
}
