use std::{rc::Rc, time::Duration};

use gpui::{
    App, BoxShadow, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, hsla, prelude::FluentBuilder,
    px,
};
use gpui_component::Icon;

use crate::{
    a11y::{Accessible, Role},
    icons::IconName,
    motion::{TOAST_IN, TOAST_OUT, TOAST_SLIDE, animate},
    theme::tokens,
    tokens::Typeset,
};

use super::button::{ButtonKind, EditorButton};

pub const TOAST_DURATION: Duration = Duration::from_millis(2500);
const RADIUS: Pixels = px(14.);
const PADDING_X: Pixels = px(16.);
const PADDING_Y: Pixels = px(11.);
const GAP: Pixels = px(10.);
/// A settings row's space between its title and its control.
const ACTION_GAP: Pixels = px(16.);
const ICON_BOX: Pixels = px(32.);
const GLYPH: Pixels = px(18.);
const MAX_WIDTH: Pixels = px(380.);
const SHADOW_INK: f32 = 0.18;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastStyle {
    #[default]
    Normal,
    Error,
}

type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The card only: whoever shows it places it, keeps it for [`TOAST_DURATION`], then marks it leaving for [`TOAST_OUT`].
#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    title: SharedString,
    message: Option<SharedString>,
    icon: Option<IconName>,
    style: ToastStyle,
    leaving: bool,
    settled: bool,
    action: Option<(SharedString, ActionHandler)>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: None,
            icon: None,
            style: ToastStyle::Normal,
            leaving: false,
            settled: false,
            action: None,
        }
    }

    /// A button on the card, such as Undo.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(on_click)));
        self
    }

    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn style(mut self, style: ToastStyle) -> Self {
        self.style = style;
        self
    }

    /// Plays the fade out; the host drops the toast once [`TOAST_OUT`] has run.
    pub fn leaving(mut self, leaving: bool) -> Self {
        self.leaving = leaving;
        self
    }

    /// Draws the card at rest with no entrance, for previews that are not being shown.
    pub fn settled(mut self) -> Self {
        self.settled = true;
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let error = self.style == ToastStyle::Error;
        let card = div()
            .id(self.id)
            .relative()
            .flex()
            .items_center()
            .gap(GAP)
            .max_w(MAX_WIDTH)
            .px(PADDING_X)
            .py(PADDING_Y)
            .rounded(RADIUS)
            .bg(t.dropdown)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), hsla(0., 0., 0., SHADOW_INK))
                    .blur_radius(px(9.))
                    .spread_radius(px(-2.)),
            ])
            .text_color(t.text)
            .accessible(
                if error { Role::Alert } else { Role::Status },
                self.title.clone(),
            )
            .when_some(self.message.clone(), |d, message| {
                d.aria_description(message)
            })
            .when_some(self.icon, |d, icon| {
                d.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(ICON_BOX)
                        .text_color(if error {
                            t.destructive
                        } else {
                            t.text_secondary
                        })
                        .child(Icon::new(icon).size(GLYPH)),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().typeset(t.type_scale.title).child(self.title))
                    .when_some(self.message, |d, message| {
                        d.child(
                            div()
                                .typeset(t.type_scale.label)
                                .text_color(t.text_secondary)
                                .line_clamp(2)
                                .child(message),
                        )
                    }),
            )
            .when_some(self.action, |d, (label, on_click)| {
                d.child(
                    div().flex_none().ml(ACTION_GAP - GAP).child(
                        EditorButton::new("toast-action")
                            .label(label)
                            .kind(ButtonKind::Bordered)
                            .on_click(move |_, window, cx| on_click(window, cx)),
                    ),
                )
            });
        if self.settled && !self.leaving {
            card.into_any_element()
        } else if self.leaving {
            animate(card, "toast-out", TOAST_OUT, cx, |card, delta| {
                card.opacity(1. - delta)
            })
        } else {
            let slide = f32::from(TOAST_SLIDE);
            animate(card, "toast-in", TOAST_IN, cx, move |card, delta| {
                card.top(px(-slide * (1. - delta))).opacity(delta)
            })
        }
    }
}
