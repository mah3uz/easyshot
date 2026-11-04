use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
    transparent_black,
};
use gpui_component::{Icon, tooltip::Tooltip};

use crate::{
    a11y::{Accessible, Role},
    components::button::{ClickHandler, Interaction, focus_halo, keyed_focus, shows_focus},
    icons::IconName,
    theme::tokens,
};

pub(crate) const SIZE: Pixels = px(28.);
pub(crate) const GLYPH: Pixels = px(14.);

/// A round, borderless glyph button for panel accessories, where a full editor button would be too loud.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    size: Pixels,
    glyph: Pixels,
    disabled: bool,
    preview: Interaction,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            label: label.into(),
            size: SIZE,
            glyph: GLYPH,
            disabled: false,
            preview: Interaction::Rest,
            on_click: None,
        }
    }

    pub fn size(mut self, size: Pixels, glyph: Pixels) -> Self {
        self.size = size;
        self.glyph = glyph;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let focus = keyed_focus(&self.id, window, cx);
        let ring = !self.disabled && shows_focus(&focus, self.preview, window);
        let live = !self.disabled && self.preview == Interaction::Rest;
        let (fill, glyph) = match (self.disabled, self.preview) {
            (true, _) => (transparent_black(), t.text_tertiary),
            (false, Interaction::Hover) => (t.hover_fill, t.text),
            (false, Interaction::Pressed) => (t.press_fill, t.text),
            _ => (transparent_black(), t.text_secondary),
        };
        let (hover_fill, press_fill, hover_glyph) = (t.hover_fill, t.press_fill, t.text);
        let radius = self.size / 2.;
        let label = self.label.clone();
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.size)
            .rounded(radius)
            .bg(fill)
            .text_color(glyph)
            .accessible(Role::Button, self.label)
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .when(live, |d| {
                d.hover(move |s| s.bg(hover_fill).text_color(hover_glyph))
                    .active(move |s| s.bg(press_fill).text_color(hover_glyph))
            })
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
            .child(Icon::new(self.icon).size(self.glyph).flex_none())
            .when(ring, |d| d.child(focus_halo(&t, radius)))
            .when_some(self.on_click.filter(|_| !self.disabled), |d, on_click| {
                d.on_click(move |event, window, cx| on_click(event, window, cx))
            })
    }
}
