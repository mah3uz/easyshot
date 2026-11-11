use std::rc::Rc;

use crate::components::Tip;
use gpui::{
    AnimationExt, AnyElement, App, ClickEvent, Div, ElementId, FocusHandle, Hsla,
    InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Transformation, Window, div, percentage,
    prelude::FluentBuilder, px, transparent_black,
};
use gpui_component::Icon;

use crate::{
    a11y::{Accessible, Role},
    icons::IconName,
    motion::{SPIN, reduce_motion},
    theme::tokens,
    tokens::{Tokens, Typeset},
};

/// A state a control can be pinned to, so a gallery can show every state at once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interaction {
    #[default]
    Rest,
    Hover,
    Pressed,
    Focused,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonKind {
    #[default]
    Plain,
    Bordered,
    Prominent,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Hsla,
    pub border: Hsla,
    pub text: Hsla,
}

pub fn look(
    t: &Tokens,
    kind: ButtonKind,
    selected: bool,
    disabled: bool,
    state: Interaction,
) -> Look {
    let none = transparent_black();
    let bordered = kind == ButtonKind::Bordered;
    let rest = Look {
        fill: if bordered { t.bordered_fill } else { none },
        border: if bordered { t.bordered_border } else { none },
        text: t.text,
    };
    if disabled && kind == ButtonKind::Prominent {
        return Look {
            fill: t.accent.opacity(0.4),
            border: none,
            text: t.on_accent.opacity(0.7),
        };
    }
    if disabled {
        return Look {
            text: t.text_secondary,
            ..rest
        };
    }
    if selected || kind == ButtonKind::Prominent {
        let alpha = match (state, selected) {
            (Interaction::Hover, false) => 0.9,
            (Interaction::Hover | Interaction::Pressed, _) => 0.8,
            _ => 1.0,
        };
        return Look {
            fill: t.accent.opacity(alpha),
            border: none,
            text: t.on_accent,
        };
    }
    match state {
        Interaction::Hover => Look {
            fill: t.hover_fill,
            border: t.hover_border,
            ..rest
        },
        Interaction::Pressed => Look {
            fill: t.press_fill,
            border: t.hover_border,
            ..rest
        },
        Interaction::Rest | Interaction::Focused => rest,
    }
}

pub(crate) type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Drawn over the control's own edge, so showing it never moves the layout.
pub(crate) fn focus_ring(t: &Tokens, radius: Pixels) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .border(t.metrics.focus_ring)
        .border_color(t.accent.opacity(0.72))
}

/// Outside the edge with a hairline gap, so it still reads around an accent fill.
pub(crate) fn focus_halo(t: &Tokens, radius: Pixels) -> Div {
    let out = t.metrics.focus_ring + t.metrics.hairline;
    focus_ring(t, radius + out)
        .top(-out)
        .left(-out)
        .right(-out)
        .bottom(-out)
}

pub(crate) fn keyed_focus(id: &ElementId, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
        .read(cx)
        .clone()
}

pub(crate) fn shows_focus(focus: &FocusHandle, preview: Interaction, window: &Window) -> bool {
    preview == Interaction::Focused
        || (focus.is_focused(window) && window.last_input_was_keyboard())
}

fn spinner(cx: &App) -> AnyElement {
    let icon = Icon::new(IconName::LoaderCircle).size(px(16.)).flex_none();
    if reduce_motion(cx) {
        return icon.into_any_element();
    }
    icon.with_animation("spin", SPIN.animation().repeat(), |icon, turn| {
        icon.transform(Transformation::rotate(percentage(turn)))
    })
    .into_any_element()
}

#[derive(IntoElement)]
pub struct EditorButton {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<IconName>,
    accessible_label: Option<SharedString>,
    tooltip: Option<SharedString>,
    kind: ButtonKind,
    selected: bool,
    toggle: bool,
    disabled: bool,
    busy: bool,
    padding_x: Option<Pixels>,
    preview: Interaction,
    on_click: Option<ClickHandler>,
}

impl EditorButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            accessible_label: None,
            tooltip: None,
            kind: ButtonKind::Plain,
            selected: false,
            toggle: false,
            disabled: false,
            busy: false,
            padding_x: None,
            preview: Interaction::Rest,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Required when the button shows only an icon.
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }

    pub fn tip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn toggle(mut self, toggle: bool) -> Self {
        self.toggle = toggle;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A turning spinner in place of the icon; pair it with `disabled`.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn padding_x(mut self, padding: Pixels) -> Self {
        self.padding_x = Some(padding);
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

impl RenderOnce for EditorButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let paint = |state| look(&t, self.kind, self.selected, self.disabled, state);
        let shown = paint(self.preview);
        let (hover, pressed) = (paint(Interaction::Hover), paint(Interaction::Pressed));
        let focus = keyed_focus(&self.id, window, cx);
        let ring = !self.disabled && shows_focus(&focus, self.preview, window);
        let live = !self.disabled && self.preview == Interaction::Rest;
        let name = self
            .accessible_label
            .clone()
            .or_else(|| self.label.clone())
            .unwrap_or_default();
        let icon_only = self.label.is_none();
        div()
            .id(self.id)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(5.))
            .h(m.button_height)
            .when(icon_only, |d| d.min_w(m.button_height))
            .px(self.padding_x.unwrap_or(m.button_padding_x))
            .rounded(m.button_radius)
            .border(m.hairline)
            .border_color(shown.border)
            .bg(shown.fill)
            .text_color(shown.text)
            .typeset(t.type_scale.tool_button)
            .accessible(Role::Button, name)
            .when(self.toggle, |d| d.toggled(self.selected))
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .when(live, |d| {
                d.hover(move |s| s.bg(hover.fill).border_color(hover.border))
                    .active(move |s| s.bg(pressed.fill).border_color(pressed.border))
            })
            .when_some(self.tooltip, |d, tip| d.tip(tip))
            .when(self.busy, |d| d.child(spinner(cx)))
            .when_some(self.icon.filter(|_| !self.busy), |d, icon| {
                d.child(Icon::new(icon).size(px(16.)).flex_none())
            })
            .when_some(self.label, |d, label| d.child(label))
            .when(ring, |d| d.child(focus_halo(&t, m.button_radius)))
            .when_some(self.on_click.filter(|_| !self.disabled), |d, on_click| {
                d.on_click(move |event, window, cx| on_click(event, window, cx))
            })
    }
}
