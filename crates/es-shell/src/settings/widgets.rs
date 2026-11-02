use es_ui::{
    a11y::{Accessible, Role},
    icons::IconName,
    theme::tokens,
    tokens::{Appearance, TYPE, Tokens},
};
use gpui::{
    AnyElement, App, Div, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Styled, div, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, Sizable, h_flex, switch::Switch, v_flex};

pub const PAGE_MAX_WIDTH: f32 = 680.;
const ROW_MIN_HEIGHT: f32 = 44.;
const CARD_RADIUS: f32 = 12.;
pub const ROW_TEXT: f32 = 13.;

pub fn card_fill(t: &Tokens) -> Hsla {
    match t.appearance {
        Appearance::Light => t.window,
        Appearance::Dark => t.bordered_fill,
    }
}

pub fn page_fill(t: &Tokens) -> Hsla {
    match t.appearance {
        Appearance::Light => t.workspace,
        Appearance::Dark => t.window,
    }
}

pub fn caption(text: impl Into<SharedString>, cx: &App) -> Div {
    let t = tokens(cx);
    div()
        .text_size(TYPE.footer.size)
        .text_color(t.text_secondary)
        .child(text.into())
}

/// A header, a card of rows split by inset rules, and a footer.
pub fn group(
    header: Option<SharedString>,
    rows: Vec<AnyElement>,
    footer: Option<SharedString>,
    cx: &App,
) -> Div {
    let t = tokens(cx);
    let count = rows.len();
    let mut card = v_flex()
        .rounded(px(CARD_RADIUS))
        .bg(card_fill(t))
        .border_1()
        .border_color(t.border)
        .overflow_hidden();
    for (i, row) in rows.into_iter().enumerate() {
        card = card.child(row);
        if i + 1 < count {
            card = card.child(div().ml(px(16.)).h(px(1.)).bg(t.separator));
        }
    }
    v_flex()
        .gap(px(8.))
        .when_some(header, |d, header| {
            d.child(
                div()
                    .px(px(4.))
                    .text_size(TYPE.footer.size)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text_secondary)
                    .child(header),
            )
        })
        .child(card)
        .when_some(footer, |d, footer| d.child(caption(footer, cx).px(px(4.))))
}

pub fn row(
    title: impl Into<SharedString>,
    help: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let t = tokens(cx);
    h_flex()
        .min_h(px(ROW_MIN_HEIGHT))
        .px(px(16.))
        .py(px(11.))
        .gap(px(16.))
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(ROW_TEXT))
                        .text_color(t.text)
                        .child(title.into()),
                )
                .when_some(help, |d, help| d.child(caption(help, cx))),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

pub fn switch(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    handler: impl Fn(&bool, &mut gpui::Window, &mut App) + 'static,
) -> Switch {
    Switch::new(id)
        .checked(on)
        .small()
        .accessibility_label(label)
        .on_click(handler)
}

pub fn status_mark(ok: bool, text: impl Into<SharedString>, cx: &App) -> Div {
    let t = tokens(cx);
    let (icon, colour) = if ok {
        (IconName::CircleCheck, t.success)
    } else {
        (IconName::CircleX, t.text_tertiary)
    };
    h_flex()
        .gap(px(6.))
        .text_size(TYPE.footer.size)
        .text_color(if ok {
            t.text_secondary
        } else {
            t.text_tertiary
        })
        .child(Icon::new(icon).size(px(14.)).text_color(colour))
        .child(text.into())
}

pub fn alert(title: SharedString, body: SharedString, cx: &App) -> AnyElement {
    let t = tokens(cx);
    h_flex()
        .id("config-error")
        .accessible(Role::Alert, title.clone())
        .items_start()
        .gap(px(10.))
        .p(px(12.))
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(t.destructive.opacity(0.5))
        .bg(t.destructive.opacity(0.08))
        .child(
            Icon::new(IconName::TriangleAlert)
                .size(px(16.))
                .text_color(t.destructive),
        )
        .child(
            v_flex()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(ROW_TEXT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(title),
                )
                .child(caption(body, cx)),
        )
        .into_any_element()
}

pub fn page(title: SharedString, children: Vec<AnyElement>, cx: &App) -> Div {
    let t = tokens(cx);
    v_flex()
        .w_full()
        .max_w(px(PAGE_MAX_WIDTH))
        .mx_auto()
        .px(px(28.))
        .pt(px(20.))
        .pb(px(28.))
        .gap(px(24.))
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(title),
        )
        .children(children)
}
