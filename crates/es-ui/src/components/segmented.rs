use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, transparent_black,
};
use gpui_component::{Icon, tooltip::Tooltip};

use crate::{
    a11y::{Accessible, Role},
    components::button::{Interaction, focus_halo, keyed_focus, shows_focus},
    icons::IconName,
    theme::tokens,
    tokens::{Appearance, Typeset},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    label: SharedString,
    icon: Option<IconName>,
    help: Option<SharedString>,
}

impl Segment {
    pub fn text(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            help: None,
        }
    }

    /// The label is still required: assistive technology reads it in place of the glyph.
    pub fn icon(icon: IconName, label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon),
            help: None,
        }
    }

    /// A tooltip for an option whose label alone does not say what it does.
    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Segmented {
    id: ElementId,
    label: SharedString,
    segments: Vec<Segment>,
    selected: usize,
    height: Option<Pixels>,
    disabled: bool,
    preview: Option<(usize, Interaction)>,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        segments: impl IntoIterator<Item = Segment>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            segments: segments.into_iter().collect(),
            selected: 0,
            height: None,
            disabled: false,
            preview: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Pins one segment, or the whole control for `Focused`, to a state.
    pub fn preview(mut self, index: usize, state: Interaction) -> Self {
        self.preview = Some((index, state));
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let inset = m.control_inset;
        let segment_radius = m.slider_radius - inset;
        let focus = keyed_focus(&self.id, window, cx);
        let pinned = |i: usize| match self.preview {
            Some((p, state)) if p == i => state,
            _ => Interaction::Rest,
        };
        let whole = self.preview.map_or(Interaction::Rest, |(_, s)| s);
        let ring = !self.disabled && shows_focus(&focus, whole, window);
        let count = self.segments.len();
        let selected = self.selected;
        let selected_border = match t.appearance {
            Appearance::Light => t.border,
            Appearance::Dark => transparent_black(),
        };
        let hover_fill = t.bordered_fill;
        let interactive = !self.disabled && self.on_select.is_some();
        let segments = self.segments.into_iter().enumerate().map(|(i, segment)| {
            let on = i == selected;
            let fill = match (on, pinned(i)) {
                (true, _) => t.selected_segment,
                (false, Interaction::Hover | Interaction::Pressed) => hover_fill,
                _ => transparent_black(),
            };
            let on_select = self.on_select.clone();
            div()
                .id(i)
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .px(px(6.))
                .rounded(segment_radius)
                .border(m.hairline)
                .border_color(if on {
                    selected_border
                } else {
                    transparent_black()
                })
                .bg(fill)
                .text_color(if on { t.text } else { t.text_secondary })
                .accessible(Role::RadioButton, segment.label.clone())
                .toggled(on)
                .aria_position_in_set(i + 1)
                .aria_size_of_set(count)
                .when(interactive && !on && pinned(i) == Interaction::Rest, |d| {
                    d.hover(move |s| s.bg(hover_fill))
                })
                .when_some(segment.help, |d, help| {
                    d.aria_description(help.clone())
                        .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
                })
                .map(|d| match segment.icon {
                    Some(icon) => d.child(Icon::new(icon).size(px(14.)).flex_none()),
                    None => d.child(div().truncate().child(segment.label)),
                })
                .when_some(on_select.filter(|_| interactive), |d, on_select| {
                    d.on_click(move |_, window, cx| {
                        if i != selected {
                            on_select(i, window, cx);
                        }
                    })
                })
        });
        let on_key = self.on_select.clone().filter(|_| interactive);
        div()
            .id(self.id)
            .relative()
            .flex()
            .w_full()
            .h(self.height.unwrap_or(m.slider_height))
            .p(inset)
            .rounded(m.slider_radius)
            .bg(t.segment_track)
            .typeset(t.type_scale.segmented)
            .accessible(Role::RadioGroup, self.label)
            .when(self.disabled, |d| d.opacity(0.48))
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .when_some(on_key, |d, on_select| {
                d.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let next = match event.keystroke.key.as_str() {
                        "left" | "up" => selected.checked_sub(1),
                        "right" | "down" => Some(selected + 1).filter(|&n| n < count),
                        _ => return,
                    };
                    cx.stop_propagation();
                    if let Some(next) = next {
                        on_select(next, window, cx);
                    }
                })
            })
            .children(segments)
            .when(ring, |d| d.child(focus_halo(&t, m.slider_radius)))
    }
}
