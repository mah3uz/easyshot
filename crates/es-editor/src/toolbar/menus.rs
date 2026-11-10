use std::rc::Rc;

use es_ui::components::Tip;
use es_ui::{
    a11y::{Accessible, Role},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    Anchor, App, Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement,
    RenderOnce, SharedString, Stateful, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div, prelude::FluentBuilder, px, transparent_black,
};
use gpui_component::{
    Icon, Selectable,
    menu::{DropdownMenu, PopupMenuItem},
};

/// The dropdown keeps its trigger's open state through `Selectable`, which a plain `Div` lacks.
#[derive(IntoElement)]
pub struct MenuTrigger {
    base: Stateful<Div>,
    open: bool,
}

impl Styled for MenuTrigger {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuTrigger {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl Selectable for MenuTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl DropdownMenu for MenuTrigger {}

impl RenderOnce for MenuTrigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hover = tokens(cx).hover_fill;
        self.base.when(self.open, |d| d.bg(hover))
    }
}

/// A borderless button face that opens a menu: an optional icon and label, then a chevron.
pub fn menu_trigger(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: Option<IconName>,
    text: Option<SharedString>,
    disabled: bool,
    cx: &App,
) -> MenuTrigger {
    let t = tokens(cx).clone();
    let m = t.metrics;
    let label: SharedString = label.into();
    let tip = label.clone();
    let (hover_fill, hover_border, press_fill) = (t.hover_fill, t.hover_border, t.press_fill);
    let base = div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(5.))
        .h(m.button_height)
        .px(px(8.))
        .rounded(m.button_radius)
        .border(m.hairline)
        .border_color(transparent_black())
        .text_color(if disabled { t.text_secondary } else { t.text })
        .typeset(t.type_scale.tool_button)
        .accessible(Role::Button, label)
        .when(!disabled, |d| {
            d.tab_index(0)
                .hover(move |s| s.bg(hover_fill).border_color(hover_border))
                .active(move |s| s.bg(press_fill))
        })
        .when(text.is_none(), |d| d.tip(tip))
        .when_some(icon, |d, icon| {
            d.child(Icon::new(icon).size(px(16.)).flex_none())
        })
        .when_some(text, |d, text| {
            d.child(div().flex_1().overflow_hidden().text_ellipsis().child(text))
        })
        .child(
            Icon::new(IconName::ChevronDown)
                .size(px(10.))
                .flex_none()
                .text_color(t.text_secondary),
        );
    MenuTrigger { base, open: false }
}

pub type Pick<T> = Rc<dyn Fn(T, &mut Window, &mut App)>;

/// A menu of labelled choices with a check on the current one.
pub fn choice_menu<T: Copy + PartialEq + 'static>(
    trigger: MenuTrigger,
    choices: Vec<(T, String, Option<IconName>)>,
    current: Option<T>,
    on_pick: Pick<T>,
) -> impl IntoElement {
    trigger.dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
        for (value, title, icon) in choices.iter().cloned() {
            let on_pick = on_pick.clone();
            let mut item = PopupMenuItem::new(title)
                .checked(Some(value) == current)
                .on_click(move |_, window, cx| on_pick(value, window, cx));
            if let Some(icon) = icon {
                item = item.icon(Icon::new(icon));
            }
            menu = menu.item(item);
        }
        menu
    })
}
