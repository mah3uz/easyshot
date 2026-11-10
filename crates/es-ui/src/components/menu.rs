use std::{path::Path, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, RenderOnce, SharedString, Stateful, StatefulInteractiveElement, Styled,
    StyledImage, Window, div, img, prelude::FluentBuilder, px, relative,
};
use gpui_component::Icon;

use crate::{
    a11y::{Accessible, Role},
    components::button::ClickHandler,
    icons::IconName,
    theme::tokens,
};

pub const MENU_ROW: f32 = 32.;
pub const MENU_SECTION: f32 = 28.;
pub const MENU_SEPARATOR: f32 = 9.;
/// Between the menu's edge and its rows, above and below.
pub const MENU_INSET: f32 = 4.;
const LEAD: f32 = 18.;

type HoverHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// The menu's box; the caller adds its height and scrolling.
pub fn menu(id: impl Into<ElementId>, label: impl Into<SharedString>, cx: &App) -> Stateful<Div> {
    let t = tokens(cx);
    div()
        .id(id)
        .accessible(Role::Menu, label)
        .flex()
        .flex_col()
        .p(px(MENU_INSET))
        .rounded(px(8.))
        .bg(t.dropdown)
        .border_1()
        .border_color(t.border)
}

/// What stands before a row's text.
#[derive(Clone)]
pub enum MenuLead {
    /// Room for a check mark, drawn when true.
    Check(bool),
    Glyph(IconName),
    /// An app's icon file; a generic window glyph stands in when there is none or it fails to load.
    App(Option<Arc<Path>>),
}

/// One choice: lead, text on one line, and quieter detail after it.
#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    label: SharedString,
    detail: Option<SharedString>,
    a11y: Option<SharedString>,
    lead: MenuLead,
    active: bool,
    on_click: Option<ClickHandler>,
    on_hover: Option<HoverHandler>,
}

impl MenuItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, lead: MenuLead) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            a11y: None,
            lead,
            active: false,
            on_click: None,
            on_hover: None,
        }
    }

    pub fn detail(mut self, detail: Option<impl Into<SharedString>>) -> Self {
        self.detail = detail.map(Into::into);
        self
    }

    /// The accessible label, when it says more than the text.
    pub fn a11y(mut self, label: impl Into<SharedString>) -> Self {
        self.a11y = Some(label.into());
        self
    }

    /// Under the pointer or chosen with the keyboard.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }
}

fn window_glyph() -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(LEAD))
        .child(Icon::new(IconName::AppWindow).size(px(16.)))
        .into_any_element()
}

impl RenderOnce for MenuItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let check = match self.lead {
            MenuLead::Check(on) => Some(on),
            _ => None,
        };
        let slot = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(LEAD))
            .text_color(t.text_secondary);
        let lead = match self.lead {
            MenuLead::Check(on) => slot.when(on, |d| {
                d.text_color(t.text)
                    .child(Icon::new(IconName::Check).size(px(12.)))
            }),
            MenuLead::Glyph(icon) => slot.child(Icon::new(icon).size(px(14.))),
            MenuLead::App(Some(path)) => slot.child(
                img(path)
                    .size(px(LEAD))
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(window_glyph),
            ),
            MenuLead::App(None) => slot.child(window_glyph()),
        };
        div()
            .id(self.id)
            .accessible(
                Role::MenuItem,
                self.a11y.unwrap_or_else(|| self.label.clone()),
            )
            .when_some(check, |d, on| d.toggled(on))
            .aria_selected(self.active)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .h(px(MENU_ROW))
            .px(px(8.))
            .rounded(t.metrics.button_radius)
            .text_color(t.text)
            .when(self.active, |d| d.bg(t.hover_fill))
            .when(self.on_hover.is_none(), |d| d.hover(|s| s.bg(t.hover_fill)))
            .child(lead)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(13.))
                    .child(self.label),
            )
            .when_some(self.detail, |d, detail| {
                d.child(
                    div()
                        .flex_none()
                        .max_w(relative(0.45))
                        .truncate()
                        .text_size(px(12.))
                        .text_color(t.text_secondary)
                        .child(detail),
                )
            })
            .when_some(self.on_hover, |d, on_hover| {
                d.on_hover(move |hovered, window, cx| on_hover(hovered, window, cx))
            })
            .when_some(self.on_click, |d, on_click| {
                d.on_click(move |event, window, cx| on_click(event, window, cx))
            })
    }
}

/// A small label over the rows that follow it.
#[derive(IntoElement)]
pub struct MenuSection {
    id: ElementId,
    label: SharedString,
}

impl MenuSection {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

impl RenderOnce for MenuSection {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let style = t.type_scale.section_header;
        div()
            .id(self.id)
            .accessible(Role::Heading, self.label.clone())
            .flex()
            .flex_none()
            .items_end()
            .h(px(MENU_SECTION))
            .px(px(8.))
            .pb(px(5.))
            .text_size(style.size)
            .font_weight(style.weight)
            .text_color(t.text_secondary)
            .child(div().truncate().child(self.label))
    }
}

/// Text in a menu that is not a choice, such as an empty list.
#[derive(IntoElement)]
pub struct MenuNote {
    id: ElementId,
    text: SharedString,
}

impl MenuNote {
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
        }
    }
}

impl RenderOnce for MenuNote {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        div()
            .id(self.id)
            .accessible(Role::Label, self.text.clone())
            .flex()
            .flex_none()
            .items_center()
            .h(px(MENU_ROW))
            .px(px(8.))
            .text_size(px(12.))
            .text_color(t.text_secondary)
            .child(div().truncate().child(self.text))
    }
}

pub fn menu_separator(cx: &App) -> Div {
    div()
        .flex_none()
        .h(px(MENU_SEPARATOR))
        .py(px(4.))
        .px(px(8.))
        .child(div().h(px(1.)).bg(tokens(cx).border))
}
