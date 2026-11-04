use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, Context, Div, ElementId, InteractiveElement, Interactivity,
    IntoElement, ParentElement, Pixels, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon, Selectable,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    tooltip::Tooltip,
};

use crate::{
    a11y::{Accessible, Role},
    components::{
        IconButton,
        button::{Interaction, focus_ring, keyed_focus, shows_focus},
        icon_button,
    },
    fl,
    icons::IconName,
    motion::{BUTTON_POP, BUTTON_POP_SCALE, tween},
    theme::tokens,
    tokens::Typeset,
};

const MENU_HEIGHT: Pixels = px(28.);
const ROW_PADDING_Y: Pixels = px(9.);
const GAP: Pixels = px(7.);

type SelectHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type MenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;
type AddWrapper = Box<dyn FnOnce(AnyElement) -> AnyElement>;

/// The menu keeps its trigger's open state through `Selectable`, which a plain `Div` lacks.
#[derive(IntoElement)]
struct Trigger {
    base: Stateful<Div>,
    open: bool,
}

impl Styled for Trigger {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for Trigger {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl DropdownMenu for Trigger {}

impl RenderOnce for Trigger {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hover = tokens(cx).hover_fill;
        self.base
            .aria_expanded(self.open)
            .when(self.open, |d| d.bg(hover))
    }
}

/// The pinned row above the inspector: a saved-preset menu, delete for the applied preset, and add.
#[derive(IntoElement)]
pub struct PresetBar {
    id: ElementId,
    presets: Vec<SharedString>,
    applied: Option<usize>,
    disabled: bool,
    preview: Interaction,
    on_select: Option<SelectHandler>,
    on_add: Option<ActionHandler>,
    on_delete: Option<IndexHandler>,
    help: Option<SharedString>,
    value: Option<SharedString>,
    menu: Option<MenuBuilder>,
    wrap_add: Option<AddWrapper>,
}

impl PresetBar {
    pub fn new(id: impl Into<ElementId>, presets: impl IntoIterator<Item = SharedString>) -> Self {
        Self {
            id: id.into(),
            presets: presets.into_iter().collect(),
            applied: None,
            disabled: false,
            preview: Interaction::Rest,
            on_select: None,
            on_add: None,
            on_delete: None,
            help: None,
            value: None,
            menu: None,
            wrap_add: None,
        }
    }

    pub fn applied(mut self, index: Option<usize>) -> Self {
        self.applied = index;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Pins the menu button to a state.
    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    /// `None` is "Current Settings": the canvas as it is, with no preset applied.
    pub fn on_select(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    pub fn on_add(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_add = Some(Rc::new(handler));
        self
    }

    pub fn on_delete(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_delete = Some(Rc::new(handler));
        self
    }

    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The accessible value; the title by default.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Replaces the default menu of saved presets.
    pub fn menu(
        mut self,
        builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.menu = Some(Rc::new(builder));
        self
    }

    /// Lets the caller anchor something to the add button, such as a popover.
    pub fn wrap_add(mut self, wrap: impl FnOnce(AnyElement) -> AnyElement + 'static) -> Self {
        self.wrap_add = Some(Box::new(wrap));
        self
    }
}

impl RenderOnce for PresetBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let focus = keyed_focus(&self.id, window, cx);
        let ring = !self.disabled && shows_focus(&focus, self.preview, window);
        let applied = self.applied.filter(|&i| i < self.presets.len());
        let title: SharedString = applied
            .map(|i| self.presets[i].clone())
            .unwrap_or_else(|| fl!("panels-presets-placeholder").into());
        let fill = match self.preview {
            Interaction::Hover => t.hover_fill,
            Interaction::Pressed => t.press_fill,
            _ => t.track,
        };
        let (hover_fill, press_fill) = (t.hover_fill, t.press_fill);
        let live = !self.disabled && self.preview == Interaction::Rest;
        let help: SharedString = self
            .help
            .clone()
            .unwrap_or_else(|| fl!("panels-presets-help").into());
        let value = self.value.clone().unwrap_or_else(|| title.clone());

        let base = div()
            .id("menu")
            .relative()
            .w_full()
            .h(MENU_HEIGHT)
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(10.))
            .pr(px(8.))
            .rounded(m.button_radius)
            .bg(fill)
            .text_color(if self.disabled {
                t.text_secondary
            } else {
                t.text
            })
            .typeset(t.type_scale.tool_button)
            .accessible(Role::ComboBox, fl!("panels-presets-a11y"))
            .aria_value(value)
            .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .when(live, |d| {
                d.hover(move |s| s.bg(hover_fill))
                    .active(move |s| s.bg(press_fill))
            })
            .child(div().flex_1().min_w_0().truncate().child(title))
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(px(12.))
                    .flex_none()
                    .text_color(t.text_secondary),
            )
            .when(ring, |d| d.child(focus_ring(&t, m.button_radius)));

        let presets = self.presets.clone();
        let on_select = self.on_select.clone();
        let custom = self.menu.clone();
        let menu: gpui::AnyElement = if self.disabled {
            base.opacity(0.48).into_any_element()
        } else {
            Trigger { base, open: false }
                .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, window, cx| {
                    if let Some(custom) = &custom {
                        return custom(menu, window, cx);
                    }
                    let item = |label: SharedString, pick: Option<usize>| {
                        let on_select = on_select.clone();
                        PopupMenuItem::new(label).checked(pick == applied).on_click(
                            move |_, window, cx| {
                                if let Some(on_select) = &on_select {
                                    on_select(pick, window, cx);
                                }
                            },
                        )
                    };
                    let mut menu = menu
                        .label(fl!("panels-presets-saved"))
                        .item(item(fl!("panels-presets-current").into(), None));
                    if presets.is_empty() {
                        menu = menu.label(fl!("panels-presets-empty"));
                    }
                    for (i, name) in presets.iter().enumerate() {
                        menu = menu.item(item(name.clone(), Some(i)));
                    }
                    menu
                })
                .into_any_element()
        };

        let deletable = applied.zip(self.on_delete);
        let last_label = window.use_keyed_state((self.id.clone(), "delete"), cx, |_, _| {
            SharedString::default()
        });
        if let Some((index, _)) = deletable {
            let label = fl!(
                "panels-presets-delete",
                name = self.presets[index].to_string()
            );
            last_label.update(cx, |l, _| *l = label.into());
        }
        let shown = tween(
            (self.id.clone(), "delete"),
            if deletable.is_some() { 1. } else { 0. },
            BUTTON_POP,
            window,
            cx,
        );
        let delete = (shown > 0.).then(|| {
            let scale = BUTTON_POP_SCALE + (1. - BUTTON_POP_SCALE) * shown;
            let button = IconButton::new("delete", IconName::Trash, last_label.read(cx).clone())
                .size(icon_button::SIZE * scale, icon_button::GLYPH * scale)
                .disabled(self.disabled)
                .when_some(deletable, |b, (index, on_delete)| {
                    b.on_click(move |_, window, cx| on_delete(index, window, cx))
                });
            div()
                .flex_none()
                .size(icon_button::SIZE)
                .flex()
                .items_center()
                .justify_center()
                .opacity(shown)
                .child(button)
        });
        let add = IconButton::new("add", IconName::Plus, fl!("panels-presets-add"))
            .disabled(self.disabled)
            .when_some(self.on_add, |b, on_add| {
                b.on_click(move |_, window, cx| on_add(window, cx))
            })
            .into_any_element();
        let add = match self.wrap_add {
            Some(wrap) => wrap(add),
            None => add,
        };

        div()
            .id(self.id)
            .w_full()
            .flex()
            .items_center()
            .gap(GAP)
            .px(m.inspector_padding)
            .py(ROW_PADDING_Y)
            // A zero intrinsic width keeps a long preset name from widening the panel; the kit's wrappers would pass it up.
            .child(div().flex_1().w(px(0.)).overflow_hidden().child(menu))
            .children(delete)
            .child(add)
    }
}
