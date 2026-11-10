use super::model::{self, SIZE_MAX, SIZE_MIN, SIZE_STEP};
use super::view::Gallery;
use crate::strings::fl;
use es_ui::components::Tip;
use es_ui::{
    a11y::{Accessible, Range, Role},
    icons::IconName,
    tokens::Tokens,
};
use gpui::{
    AccessibleAction, AnyElement, Context, CursorStyle, DispatchPhase, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, ScrollDelta, ScrollEvent, StatefulInteractiveElement, Styled, Window,
    canvas, div, prelude::FluentBuilder, px,
};
use gpui_component::Icon;

const TRACK_WIDTH: f32 = 84.;
const HIT_HEIGHT: f32 = 20.;
const RAIL: f32 = 4.;
const KNOB: f32 = 14.;

pub fn default_size() -> u32 {
    es_model::config::GalleryCfg::default().item_size
}

/// The size a pointer at `x` across a track of `width` asks for; the knob's centre never leaves the track.
fn size_at(x: f32, width: f32) -> u32 {
    let travel = (width - KNOB).max(1.);
    let fraction = ((x - KNOB / 2.) / travel).clamp(0., 1.);
    SIZE_MIN + (fraction * (SIZE_MAX - SIZE_MIN) as f32).round() as u32
}

impl Gallery {
    /// Lines are wheel notches, one size step each; pixels come from a touchpad and move the size one for one.
    pub fn zoom_by_wheel(&mut self, delta: ScrollDelta, cx: &mut Context<Self>) {
        let by = match delta {
            ScrollDelta::Lines(lines) => lines.y * SIZE_STEP as f32,
            ScrollDelta::Pixels(pixels) => pixels.y.as_f32(),
        };
        let size = self.item_size(cx) as f32 + by;
        self.set_item_size(size.round().max(0.) as u32, cx);
    }

    /// Ctrl with `=`/`+`, `-` and `0`, as in a browser.
    pub(super) fn size_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let size = self.item_size(cx);
        let next = match key {
            "=" | "+" => model::size_step(size, true),
            "-" => model::size_step(size, false),
            "0" => default_size(),
            _ => return false,
        };
        self.set_item_size(next, cx);
        true
    }

    fn drag_size(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let track = self.size_track.get();
        let size = size_at((x - track.left()).as_f32(), track.size.width.as_f32());
        self.set_item_size(size, cx);
    }

    /// Captures Ctrl+wheel over the grid before the list scrolls with it.
    pub(super) fn wheel_zoom(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity().downgrade();
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.on_mouse_event(move |event: &ScrollEvent, phase, _, cx| {
                    if phase != DispatchPhase::Capture
                        || !event.modifiers.control
                        || !bounds.contains(&event.position)
                    {
                        return;
                    }
                    cx.stop_propagation();
                    let delta = event.delta;
                    let _ = view.update(cx, |this, cx| this.zoom_by_wheel(delta, cx));
                });
            },
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    pub(super) fn size_slider(
        &self,
        t: &Tokens,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let size = self.item_size(cx);
        let progress = (size - SIZE_MIN) as f32 / (SIZE_MAX - SIZE_MIN) as f32;
        let knob_x = progress * (TRACK_WIDTH - KNOB);
        let focused = self.size_focus.is_focused(window);
        let (track, dragging, entity) = (self.size_track.clone(), self.size_dragging, cx.entity());
        let pointer = canvas(
            move |bounds, _, _| track.set(bounds),
            move |_, (), window, _| {
                if !dragging {
                    return;
                }
                let moving = entity.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        moving.update(cx, |this, cx| this.drag_size(event.position.x, cx));
                    }
                });
                let ending = entity.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        ending.update(cx, |this, cx| {
                            this.size_dragging = false;
                            cx.notify();
                        });
                    }
                });
            },
        )
        .absolute()
        .size_full();
        let rail_top = px((HIT_HEIGHT - RAIL) / 2.);
        let help = fl!("gallery-size-help");
        let step = |bigger: bool| {
            let view = cx.entity();
            move |_: Option<&_>, _: &mut Window, cx: &mut gpui::App| {
                view.update(cx, |this, cx| {
                    let next = model::size_step(this.item_size(cx), bigger);
                    this.set_item_size(next, cx);
                })
            }
        };
        let slider = div()
            .id("item-size")
            .accessible(Role::Slider, fl!("gallery-size"))
            .range(Range {
                value: f64::from(size),
                min: f64::from(SIZE_MIN),
                max: f64::from(SIZE_MAX),
                step: f64::from(SIZE_STEP),
            })
            .aria_value(fl!("gallery-size-value", size = size))
            .track_focus(&self.size_focus)
            .tab_index(0)
            .relative()
            .flex_none()
            .w(px(TRACK_WIDTH))
            .h(px(HIT_HEIGHT))
            .cursor(CursorStyle::PointingHand)
            .tip(help)
            .child(pointer)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(rail_top)
                    .h(px(RAIL))
                    .rounded_full()
                    .bg(t.track),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(rail_top)
                    .h(px(RAIL))
                    .w(px(knob_x + KNOB / 2.))
                    .rounded_full()
                    .bg(t.accent),
            )
            .child(
                div()
                    .absolute()
                    .left(px(knob_x))
                    .top(px((HIT_HEIGHT - KNOB) / 2.))
                    .size(px(KNOB))
                    .rounded_full()
                    .bg(t.on_accent)
                    .shadow_sm()
                    .border_1()
                    .border_color(t.bordered_border)
                    .when(focused || dragging, |d| {
                        d.border(t.metrics.focus_ring).border_color(t.accent)
                    }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.size_focus, cx);
                    this.size_dragging = true;
                    this.drag_size(event.position.x, cx);
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                let bigger = match event.keystroke.key.as_str() {
                    "left" | "down" => false,
                    "right" | "up" => true,
                    _ => return,
                };
                cx.stop_propagation();
                let next = model::size_step(this.item_size(cx), bigger);
                this.set_item_size(next, cx);
            }))
            .on_a11y_action(AccessibleAction::Increment, step(true))
            .on_a11y_action(AccessibleAction::Decrement, step(false));
        let glyph = |size: f32| {
            Icon::new(IconName::Image)
                .size(px(size))
                .text_color(t.text_secondary)
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .child(glyph(11.))
            .child(slider)
            .child(glyph(16.))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_track_ends_reach_the_size_range_ends() {
        let w = TRACK_WIDTH;
        assert_eq!(size_at(0., w), SIZE_MIN);
        assert_eq!(size_at(KNOB / 2., w), SIZE_MIN);
        assert_eq!(size_at(w / 2., w), (SIZE_MIN + SIZE_MAX) / 2);
        assert_eq!(size_at(w - KNOB / 2., w), SIZE_MAX);
        assert_eq!(size_at(w + 40., w), SIZE_MAX);
    }
}
