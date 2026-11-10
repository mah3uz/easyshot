use std::rc::Rc;

use crate::components::Tip;
use gpui::{
    App, ElementId, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, transparent_black,
};

use crate::{
    a11y::{Accessible, Role},
    components::button::{Interaction, focus_halo, keyed_focus, shows_focus},
    fl,
    theme::tokens,
    tokens::Appearance,
};

pub const STROKE_WIDTHS: [f32; 5] = [2., 4., 6., 8., 12.];

/// A dot grows with its width but stops at 13 px, so the widest still fits a 24 px control.
pub fn dot_diameter(width: f32) -> Pixels {
    px((width + 2.).min(13.))
}

type WidthHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct StrokeDots {
    id: ElementId,
    label: SharedString,
    widths: Vec<f32>,
    selected: f32,
    disabled: bool,
    preview: Option<(usize, Interaction)>,
    on_select: Option<WidthHandler>,
}

impl StrokeDots {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            widths: STROKE_WIDTHS.to_vec(),
            selected: STROKE_WIDTHS[1],
            disabled: false,
            preview: None,
            on_select: None,
        }
    }

    pub fn widths(mut self, widths: impl IntoIterator<Item = f32>) -> Self {
        self.widths = widths.into_iter().collect();
        self
    }

    /// A width that is not one of the dots leaves none selected.
    pub fn selected(mut self, width: f32) -> Self {
        self.selected = width;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Pins one dot, or the whole control for `Focused`, to a state.
    pub fn preview(mut self, index: usize, state: Interaction) -> Self {
        self.preview = Some((index, state));
        self
    }

    pub fn on_select(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for StrokeDots {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let inset = m.control_inset;
        let focus = keyed_focus(&self.id, window, cx);
        let pinned = |i: usize| match self.preview {
            Some((p, state)) if p == i => state,
            _ => Interaction::Rest,
        };
        let whole = self.preview.map_or(Interaction::Rest, |(_, s)| s);
        let ring = !self.disabled && shows_focus(&focus, whole, window);
        let widths = self.widths;
        let count = widths.len();
        let selected = widths
            .iter()
            .position(|w| (w - self.selected).abs() < f32::EPSILON);
        let selected_border = match t.appearance {
            Appearance::Light => t.border,
            Appearance::Dark => transparent_black(),
        };
        let hover_fill = t.bordered_fill;
        let interactive = !self.disabled && self.on_select.is_some();
        let dots: Vec<_> = widths
            .iter()
            .copied()
            .enumerate()
            .map(|(i, width)| {
                let on = selected == Some(i);
                let fill = match (on, pinned(i)) {
                    (true, _) => t.selected_segment,
                    (false, Interaction::Hover | Interaction::Pressed) => hover_fill,
                    _ => transparent_black(),
                };
                let pixels = width.round() as i64;
                let help = fl!("picks-stroke-width-help", width = pixels);
                let on_select = self.on_select.clone();
                div()
                    .id(i)
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(m.slider_radius - inset)
                    .border(m.hairline)
                    .border_color(if on {
                        selected_border
                    } else {
                        transparent_black()
                    })
                    .bg(fill)
                    .accessible(Role::RadioButton, fl!("picks-stroke-width", width = pixels))
                    .toggled(on)
                    .aria_position_in_set(i + 1)
                    .aria_size_of_set(count)
                    .when(interactive && !on && pinned(i) == Interaction::Rest, |d| {
                        d.hover(move |s| s.bg(hover_fill))
                    })
                    .when(interactive, |d| d.tip(help))
                    .child(
                        div()
                            .size(dot_diameter(width))
                            .flex_none()
                            .rounded_full()
                            .bg(if on { t.text } else { t.text_secondary }),
                    )
                    .when_some(on_select.filter(|_| interactive), |d, on_select| {
                        d.on_click(move |_, window, cx| {
                            if !on {
                                on_select(width, window, cx);
                            }
                        })
                    })
            })
            .collect();
        let on_key = self.on_select.filter(|_| interactive);
        div()
            .id(self.id)
            .relative()
            .flex()
            .w_full()
            .h(m.control_height)
            .p(inset)
            .rounded(m.slider_radius)
            .bg(t.segment_track)
            .accessible(Role::RadioGroup, self.label)
            .when(self.disabled, |d| d.opacity(0.48))
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .when_some(on_key, |d, on_select| {
                d.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let next = match event.keystroke.key.as_str() {
                        "left" | "up" => selected.map_or(Some(0), |s| s.checked_sub(1)),
                        "right" | "down" => {
                            Some(selected.map_or(0, |s| s + 1)).filter(|&n| n < count)
                        }
                        _ => return,
                    };
                    cx.stop_propagation();
                    if let Some(next) = next.filter(|&n| Some(n) != selected) {
                        on_select(widths[next], window, cx);
                    }
                })
            })
            .children(dots)
            .when(ring, |d| d.child(focus_halo(&t, m.slider_radius)))
    }
}
