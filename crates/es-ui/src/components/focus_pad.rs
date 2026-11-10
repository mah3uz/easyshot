use std::rc::Rc;

use crate::components::Tip;
use gpui::{
    App, Bounds, DispatchPhase, ElementId, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, point,
    prelude::FluentBuilder, px,
};

use crate::{
    a11y::{Accessible, Role},
    components::button::{Interaction, focus_halo, keyed_focus, shows_focus},
    fl,
    motion::{Clocked, KNOB_GROW, SETTLE, tween},
    theme::tokens,
};

pub const FOCUS_PAD_HEIGHT: Pixels = px(118.);
const KNOB: Pixels = px(10.);
const KNOB_DRAGGING: Pixels = px(12.);
const KNOB_EDGE: Pixels = px(1.5);
const NUDGE: f32 = 0.05;
const FINE_NUDGE: f32 = 0.01;

pub const CENTRE: Point<f32> = Point { x: 0.5, y: 0.5 };

#[derive(Default)]
struct Drag {
    dragging: bool,
    bounds: Bounds<Pixels>,
    recentre: Option<(Point<f32>, Clocked)>,
}

fn unit_at(bounds: Bounds<Pixels>, at: Point<Pixels>) -> Point<f32> {
    let w = f32::from(bounds.size.width).max(1.);
    let h = f32::from(bounds.size.height).max(1.);
    point(
        (f32::from(at.x - bounds.left()) / w).clamp(0., 1.),
        (f32::from(at.y - bounds.top()) / h).clamp(0., 1.),
    )
}

pub(crate) fn nudge(position: Point<f32>, key: &str, fine: bool) -> Option<Point<f32>> {
    let step = if fine { FINE_NUDGE } else { NUDGE };
    let (dx, dy) = match key {
        "left" => (-step, 0.),
        "right" => (step, 0.),
        "up" => (0., -step),
        "down" => (0., step),
        _ => return None,
    };
    Some(point(
        (position.x + dx).clamp(0., 1.),
        (position.y + dy).clamp(0., 1.),
    ))
}

type ChangeHandler = Rc<dyn Fn(Point<f32>, &mut Window, &mut App)>;

/// A point picked on a unit square: drag the knob, nudge it with the arrows, or double-click to centre it.
#[derive(IntoElement)]
pub struct FocusPad {
    id: ElementId,
    label: SharedString,
    position: Point<f32>,
    preview: Interaction,
    value: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl FocusPad {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            position: CENTRE,
            preview: Interaction::Rest,
            value: None,
            on_change: None,
        }
    }

    /// Both coordinates run from 0 at the top left to 1 at the bottom right.
    pub fn position(mut self, position: Point<f32>) -> Self {
        self.position = point(position.x.clamp(0., 1.), position.y.clamp(0., 1.));
        self
    }

    /// `Pressed` pins the dragging knob, `Hover` the lit pad and `Focused` the keyboard ring.
    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    /// Replaces the spoken "X n, Y n" when the caller can say more about the point.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(Point<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FocusPad {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let radius = m.slider_radius;
        let focus = keyed_focus(&self.id, window, cx);
        let ring = shows_focus(&focus, self.preview, window);
        let drag = window.use_keyed_state(self.id.clone(), cx, |_, _| Drag::default());
        let dragging = drag.read(cx).dragging || self.preview == Interaction::Pressed;
        let position = self.position;
        let knob = px(tween(
            (self.id.clone(), "knob"),
            f32::from(if dragging { KNOB_DRAGGING } else { KNOB }),
            KNOB_GROW,
            window,
            cx,
        ));
        let shown = match drag.read(cx).recentre {
            Some((from, clocked)) => {
                let t = clocked.progress(window, cx);
                if t >= 1. {
                    drag.update(cx, |d, _| d.recentre = None);
                }
                point(
                    from.x + (position.x - from.x) * t,
                    from.y + (position.y - from.y) * t,
                )
            }
            None => position,
        };
        let (rest_fill, hover_fill) = (t.text.opacity(0.035), t.text.opacity(0.055));
        let fill = match self.preview {
            Interaction::Hover | Interaction::Pressed => hover_fill,
            _ => rest_fill,
        };
        let percent = |v: f32| (v * 100.).round() as i32;
        let value = self.value.clone().unwrap_or_else(|| {
            fl!(
                "panels-focus-value",
                x = percent(position.x),
                y = percent(position.y)
            )
            .into()
        });
        let on_change = self.on_change.clone();

        let tracker = {
            let drag = drag.clone();
            let on_change = on_change.clone();
            canvas(
                {
                    let drag = drag.clone();
                    move |bounds, _, cx| drag.update(cx, |d, _| d.bounds = bounds)
                },
                move |_, _, window, cx| {
                    let (Some(on_change), true) = (on_change, drag.read(cx).dragging) else {
                        return;
                    };
                    let moving = drag.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && event.dragging() {
                            let bounds = moving.read(cx).bounds;
                            on_change(unit_at(bounds, event.position), window, cx);
                        }
                    });
                    let ending = drag.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            ending.update(cx, |d, cx| {
                                d.dragging = false;
                                cx.notify();
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        };

        let line = |horizontal: bool| {
            let d = div().absolute().bg(t.border);
            if horizontal {
                d.left_0().right_0().top(relative_half()).h(m.hairline)
            } else {
                d.top_0().bottom_0().left(relative_half()).w(m.hairline)
            }
        };

        let knob_el = div()
            .absolute()
            .left(gpui::relative(shown.x))
            .top(gpui::relative(shown.y))
            .ml(-knob / 2.)
            .mt(-knob / 2.)
            .size(knob)
            .rounded_full()
            .bg(t.accent)
            .border(KNOB_EDGE)
            .border_color(t.on_accent.opacity(0.9));

        let help = fl!("panels-focus-help");
        div()
            .id(self.id)
            .relative()
            .w_full()
            .h(FOCUS_PAD_HEIGHT)
            .rounded(radius)
            .bg(fill)
            .border(m.hairline)
            .border_color(t.border)
            .accessible(Role::Slider, self.label)
            .aria_value(value)
            .aria_description(help.clone())
            .track_focus(&focus)
            .tab_index(0)
            .tip(help)
            .when(self.preview == Interaction::Rest, |d| {
                d.hover(move |s| s.bg(hover_fill))
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .overflow_hidden()
                    .child(line(true))
                    .child(line(false))
                    .child(knob_el),
            )
            .child(tracker)
            .when(ring, |d| d.child(focus_halo(&t, radius)))
            .when_some(on_change, |d, on_change| {
                let pressing = on_change.clone();
                let drag = drag.clone();
                d.on_mouse_down(
                    MouseButton::Left,
                    move |event: &MouseDownEvent, window, cx| {
                        let bounds = drag.read(cx).bounds;
                        let at = if event.click_count >= 2 {
                            CENTRE
                        } else {
                            unit_at(bounds, event.position)
                        };
                        let recentre = event.click_count >= 2;
                        drag.update(cx, |d, cx| {
                            d.dragging = !recentre;
                            d.recentre = recentre
                                .then(|| Clocked::start(SETTLE, cx).map(|c| (position, c)))
                                .flatten();
                            cx.notify();
                        });
                        pressing(at, window, cx);
                    },
                )
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let keystroke = &event.keystroke;
                    let Some(next) =
                        nudge(position, keystroke.key.as_str(), keystroke.modifiers.shift)
                    else {
                        return;
                    };
                    cx.stop_propagation();
                    on_change(next, window, cx);
                })
            })
    }
}

fn relative_half() -> gpui::DefiniteLength {
    gpui::relative(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudges_move_by_a_twentieth_or_a_hundredth_and_stay_on_the_pad() {
        let lands = |from: (f32, f32), key, fine, want: (f32, f32)| {
            let got = nudge(point(from.0, from.1), key, fine).expect("an arrow nudges");
            assert!(
                (got.x - want.0).abs() < 1e-5 && (got.y - want.1).abs() < 1e-5,
                "{key} from {from:?} landed on {got:?}"
            );
        };
        lands((0.5, 0.5), "right", false, (0.55, 0.5));
        lands((0.5, 0.5), "up", true, (0.5, 0.49));
        lands((0.02, 1.), "left", false, (0., 1.));
        lands((0.5, 1.), "down", false, (0.5, 1.));
        assert_eq!(nudge(point(0.5, 0.5), "enter", false), None);
    }
}
