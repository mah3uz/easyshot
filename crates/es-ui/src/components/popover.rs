use std::rc::Rc;

use gpui::{
    Anchor, AnyElement, App, BoxShadow, ElementId, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window,
    div, hsla, prelude::FluentBuilder, px,
};
use gpui_component::{Selectable, popover::Popover as KitPopover};

use crate::{
    a11y::{Accessible, Role},
    motion::{POPOVER_OPEN, animate},
    theme::tokens,
};

const DEFAULT_WIDTH: Pixels = px(260.);
const PADDING: Pixels = px(12.);
const GAP_TO_TRIGGER: Pixels = px(6.);
const START_SCALE: f32 = 0.96;
const START_TRAVEL: f32 = 4.;
const SHADOW_INK: f32 = 0.18;

pub fn shadow(strength: f32) -> Vec<BoxShadow> {
    vec![
        BoxShadow::new(px(0.), px(6.), hsla(0., 0., 0., SHADOW_INK * strength))
            .blur_radius(px(9.))
            .spread_radius(px(-2.)),
    ]
}

/// The kit's popover keeps its trigger's open state through `Selectable`, which an arbitrary element lacks.
#[derive(IntoElement)]
struct Trigger {
    child: AnyElement,
    open: bool,
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

impl RenderOnce for Trigger {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id("trigger")
            .aria_expanded(self.open)
            .child(self.child)
    }
}

type ContentBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A panel anchored to its trigger that grows in from its anchor; Esc or a click outside closes it.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    label: SharedString,
    anchor: Anchor,
    width: Pixels,
    default_open: bool,
    open: Option<bool>,
    trigger: Option<AnyElement>,
    content: Option<ContentBuilder>,
    on_open_change: Option<OpenHandler>,
    focus: Option<FocusHandle>,
    settled: bool,
}

impl Popover {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            anchor: Anchor::TopLeft,
            width: DEFAULT_WIDTH,
            default_open: false,
            open: None,
            trigger: None,
            content: None,
            on_open_change: None,
            focus: None,
            settled: false,
        }
    }

    /// The panel's own corner that meets the trigger: `TopCenter` opens below and centred.
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = width;
        self
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Controls the open state; pair it with `on_open_change`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn trigger(mut self, trigger: impl IntoElement) -> Self {
        self.trigger = Some(trigger.into_any_element());
        self
    }

    pub fn content<E: IntoElement>(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.content = Some(Rc::new(move |window, cx| {
            content(window, cx).into_any_element()
        }));
        self
    }

    /// Takes focus as it opens, so the content's own keys work without a click first.
    pub fn focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    /// Opens at rest with no growth, for previews that are not being opened.
    pub fn settled(mut self) -> Self {
        self.settled = true;
        self
    }

    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Popover {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (label, width, content, settled) = (self.label, self.width, self.content, self.settled);
        let toward_anchor = match self.anchor {
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => START_TRAVEL,
            _ => -START_TRAVEL,
        };
        KitPopover::new(self.id)
            .appearance(false)
            .anchor(self.anchor)
            .offset(GAP_TO_TRIGGER)
            .default_open(self.default_open)
            .when_some(self.open, |p, open| p.open(open))
            .when_some(self.on_open_change, |p, handler| {
                p.on_open_change(move |open, window, cx| handler(*open, window, cx))
            })
            .when_some(self.focus, |p, handle| p.track_focus(&handle))
            .when_some(self.trigger, |p, child| {
                p.trigger(Trigger { child, open: false })
            })
            .content(move |_, window, cx| {
                let t = tokens(cx).clone();
                let m = t.metrics;
                let surface = div()
                    .id("surface")
                    .relative()
                    .w(width)
                    .flex()
                    .flex_col()
                    .p(PADDING)
                    .bg(t.dropdown)
                    .text_color(t.text)
                    .rounded(m.popover_radius)
                    .border(m.hairline)
                    .border_color(t.border)
                    .accessible(Role::Group, label.clone())
                    .when_some(content.clone(), |d, content| d.child(content(window, cx)));
                if settled {
                    return surface.shadow(shadow(1.)).into_any_element();
                }
                animate(surface, "open", POPOVER_OPEN, cx, move |surface, delta| {
                    let scale = START_SCALE + (1. - START_SCALE) * delta;
                    surface
                        .w(width * scale)
                        .top(px(toward_anchor * (1. - delta)))
                        .opacity(delta)
                        .shadow(shadow(delta * delta * delta))
                })
            })
    }
}
