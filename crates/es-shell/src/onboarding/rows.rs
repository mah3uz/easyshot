use es_ui::{
    a11y::{Accessible, Role},
    icons::IconName,
    tokens::Tokens,
};
use gpui::{
    AnyElement, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, div, prelude::FluentBuilder, px,
};
use gpui_component::Icon;

use super::{Check, CheckRow};
use crate::strings::fl;

pub(super) fn group(t: &Tokens) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(10.))
        .bg(t.bordered_fill)
        .border(t.metrics.hairline)
        .border_color(t.border)
}

pub(super) fn form_row(t: &Tokens, divided: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(14.))
        .py(px(9.))
        .when(divided, |d| {
            d.border_t(t.metrics.hairline).border_color(t.separator)
        })
}

pub(super) fn row_title(text: String) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .child(text)
}

pub(super) fn caption(t: &Tokens, text: String) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(t.text_secondary)
        .child(text)
        .into_any_element()
}

fn check_text(check: Check) -> (String, String, String) {
    match check {
        Check::Capture => (
            fl!("onboarding-check-capture"),
            fl!("onboarding-check-capture-help"),
            fl!("onboarding-check-capture-fix"),
        ),
        Check::LayerShell => (
            fl!("onboarding-check-layer-shell"),
            fl!("onboarding-check-layer-shell-help"),
            fl!("onboarding-check-layer-shell-fix"),
        ),
        Check::Clipboard => (
            fl!("onboarding-check-clipboard"),
            fl!("onboarding-check-clipboard-help"),
            fl!("onboarding-check-clipboard-fix"),
        ),
        Check::Tray => (
            fl!("onboarding-check-tray"),
            fl!("onboarding-check-tray-help"),
            fl!("onboarding-check-tray-fix"),
        ),
        Check::Recorder => (
            fl!("onboarding-check-recorder"),
            fl!("onboarding-check-recorder-help"),
            fl!("onboarding-check-recorder-fix"),
        ),
        Check::Ocr => (
            fl!("onboarding-check-ocr"),
            fl!("onboarding-check-ocr-help"),
            fl!("onboarding-check-ocr-fix"),
        ),
    }
}

/// The help line while ready; the fix while missing.
pub fn check_caption(row: CheckRow) -> String {
    let (_, help, fix) = check_text(row.check);
    if row.ready { help } else { fix }
}

pub(super) fn check_row(t: &Tokens, row: CheckRow, divided: bool) -> Div {
    let (title, _, _) = check_text(row.check);
    let (icon, tint, status) = match (row.ready, row.required) {
        (true, _) => (
            IconName::CircleCheck,
            t.success,
            fl!("onboarding-check-ready"),
        ),
        (false, true) => (
            IconName::CircleAlert,
            t.destructive,
            fl!("onboarding-check-missing"),
        ),
        (false, false) => (
            IconName::CircleAlert,
            t.text_secondary,
            fl!("onboarding-check-missing"),
        ),
    };
    let name: SharedString = format!("{title}: {status}").into();
    form_row(t, divided)
        .items_start()
        .child(
            div()
                .pt(px(1.))
                .text_color(tint)
                .child(Icon::new(icon).size(px(16.))),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(row_title(title))
                        .when(row.required, |d| {
                            d.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(t.text_secondary)
                                    .child(fl!("onboarding-check-required")),
                            )
                        }),
                )
                .child(caption(t, check_caption(row))),
        )
        .child(
            div()
                .id(SharedString::from(format!(
                    "onboarding-check-{:?}",
                    row.check
                )))
                .accessible(Role::Status, name)
                .text_size(px(12.))
                .text_color(tint)
                .child(status),
        )
}
