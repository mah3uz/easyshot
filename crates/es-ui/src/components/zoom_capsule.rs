use std::rc::Rc;

use gpui::{
    Anchor, App, Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement,
    RenderOnce, Stateful, StatefulInteractiveElement, StyleRefinement, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon, Selectable,
    menu::{DropdownMenu, PopupMenuItem},
};

use crate::{
    a11y::{Accessible, Role},
    components::button::{Interaction, focus_halo, keyed_focus, shows_focus},
    fl,
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};

pub const MIN_PERCENT: u32 = 10;
pub const MAX_PERCENT: u32 = 400;
pub const PRESETS: [u32; 5] = [25, 50, 100, 200, 400];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomCommand {
    In,
    Out,
    Fit,
    Percent(u32),
}

type CommandHandler = Rc<dyn Fn(ZoomCommand, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ZoomCapsule {
    id: ElementId,
    percent: u32,
    preview: Interaction,
    on_command: Option<CommandHandler>,
}

impl ZoomCapsule {
    pub fn new(id: impl Into<ElementId>, percent: u32) -> Self {
        Self {
            id: id.into(),
            percent,
            preview: Interaction::Rest,
            on_command: None,
        }
    }

    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_command(
        mut self,
        handler: impl Fn(ZoomCommand, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_command = Some(Rc::new(handler));
        self
    }
}

/// The dropdown keeps its trigger's open state through `Selectable`, which a plain `Div` lacks.
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
        self.base.when(self.open, |d| d.bg(hover))
    }
}

impl RenderOnce for ZoomCapsule {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let percent = self.percent;
        let focus = keyed_focus(&self.id, window, cx);
        let ring = shows_focus(&focus, self.preview, window);
        let fill = match self.preview {
            Interaction::Hover => t.hover_fill,
            Interaction::Pressed => t.press_fill,
            _ => t.window,
        };
        let (hover_fill, press_fill) = (t.hover_fill, t.press_fill);
        let live = self.preview == Interaction::Rest;
        let base = div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .p(px(8.))
            .rounded_full()
            .border(t.metrics.hairline)
            .border_color(t.border)
            .bg(fill)
            .text_color(t.accent)
            .typeset(crate::tokens::TextStyle {
                tabular: true,
                ..t.type_scale.footer
            })
            .accessible(Role::Button, fl!("controls-zoom-a11y", percent = percent))
            .track_focus(&focus)
            .tab_index(0)
            .when(live, |d| {
                d.hover(move |s| s.bg(hover_fill))
                    .active(move |s| s.bg(press_fill))
            })
            .child(
                div()
                    .w(px(40.))
                    .flex()
                    .justify_end()
                    .child(fl!("controls-zoom-percent", percent = percent)),
            )
            .child(Icon::new(IconName::ChevronDown).size(px(12.)).flex_none())
            .when(ring, |d| d.child(focus_halo(&t, px(999.))));
        let on_command = self.on_command;
        Trigger { base, open: false }.dropdown_menu_with_anchor(
            Anchor::BottomLeft,
            move |menu, _, _| {
                let item = |label: String, command: ZoomCommand| {
                    let on_command = on_command.clone();
                    PopupMenuItem::new(label).on_click(move |_, window, cx| {
                        if let Some(on_command) = &on_command {
                            on_command(command, window, cx);
                        }
                    })
                };
                let mut menu = menu
                    .item(
                        item(fl!("controls-zoom-in"), ZoomCommand::In)
                            .disabled(percent >= MAX_PERCENT),
                    )
                    .item(
                        item(fl!("controls-zoom-out"), ZoomCommand::Out)
                            .disabled(percent <= MIN_PERCENT),
                    )
                    .separator()
                    .item(item(fl!("controls-zoom-fit"), ZoomCommand::Fit));
                for preset in PRESETS {
                    menu = menu.item(
                        item(
                            fl!("controls-zoom-percent", percent = preset),
                            ZoomCommand::Percent(preset),
                        )
                        .checked(preset == percent),
                    );
                }
                menu
            },
        )
    }
}
