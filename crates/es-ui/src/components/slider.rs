use std::{cell::Cell, ops::RangeInclusive, rc::Rc};

use crate::components::Tip;
use gpui::{
    AccessibleAction, App, AppContext, Bounds, Context, Corners, CursorStyle, DispatchPhase,
    Entity, EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, canvas, div,
    fill, point, prelude::FluentBuilder, px, size,
};
use gpui_component::input::{Escape, Input, InputEvent, InputState, MoveDown, MoveUp};

use crate::{
    a11y::{Accessible, Range, Role},
    components::{
        button::{Interaction, focus_ring},
        value_format::ValueFormat,
    },
    fl,
    motion::{SLIDER_MARKERS, animate},
    theme::tokens,
    tokens::{Tokens, Typeset},
};

const DETENT: f32 = 3.;
const GAP: Pixels = px(7.);
const LABEL_INSET: Pixels = px(10.);
const TICKS: usize = 7;

/// Drags arrive as `DragStarted`, `Changed`…, `DragEnded`, so a caller can fold one drag into one undo step;
/// a `Changed` outside that pair is a single keyboard or typed edit.
#[derive(Clone, Debug, PartialEq)]
pub enum SliderEvent {
    DragStarted,
    Changed(f64),
    DragEnded,
}

pub struct InspectorSlider {
    label: SharedString,
    label_in_track: bool,
    value: f64,
    range: RangeInclusive<f64>,
    format: ValueFormat,
    disabled: bool,
    track_focus: FocusHandle,
    field: Entity<InputState>,
    baseline: String,
    hovered: bool,
    dragging: bool,
    revealed: bool,
    reveals: usize,
    track: Rc<Cell<Bounds<Pixels>>>,
    preview: Interaction,
    _field_events: Subscription,
}

impl EventEmitter<SliderEvent> for InspectorSlider {}

impl Focusable for InspectorSlider {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.track_focus.clone()
    }
}

fn zero_fraction(range: &RangeInclusive<f64>) -> Option<f64> {
    let (lo, hi) = (*range.start(), *range.end());
    (lo.is_finite() && hi.is_finite() && lo < 0. && hi > 0.).then(|| -lo / (hi - lo))
}

/// Pointer drags stay continuous; on a signed range a small sticky zone lands exactly on zero, and each side is
/// remapped around it so values next to zero stay reachable.
fn value_at(range: &RangeInclusive<f64>, x: f32, width: f32) -> Option<f64> {
    if width <= 0. {
        return None;
    }
    let (lo, hi) = (*range.start(), *range.end());
    if let Some(zero) = zero_fraction(range) {
        let zero_x = zero as f32 * width;
        let (left, right) = (zero_x - DETENT, width - zero_x - DETENT);
        if left > 0. && right > 0. {
            let value = if (x - zero_x).abs() <= DETENT {
                0.
            } else if x < zero_x {
                lo * f64::from(1. - (x / left).clamp(0., 1.))
            } else {
                hi * f64::from(((x - zero_x - DETENT) / right).clamp(0., 1.))
            };
            return Some(value + 0.);
        }
    }
    Some(lo + f64::from((x / width).clamp(0., 1.)) * (hi - lo))
}

impl InspectorSlider {
    pub fn new(
        label: impl Into<SharedString>,
        value: f64,
        range: RangeInclusive<f64>,
        format: ValueFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| InputState::new(window, cx));
        let _field_events = cx.subscribe_in(&field, window, Self::on_field);
        let mut this = Self {
            label: label.into(),
            label_in_track: true,
            value: value.clamp(*range.start(), *range.end()),
            range,
            format,
            disabled: false,
            track_focus: cx.focus_handle().tab_stop(true),
            field,
            baseline: String::new(),
            hovered: false,
            dragging: false,
            revealed: false,
            reveals: 0,
            track: Rc::default(),
            preview: Interaction::Rest,
            _field_events,
        };
        this.sync_field(window, cx);
        this
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// For a slider whose row already shows its name.
    pub fn without_track_label(mut self) -> Self {
        self.label_in_track = false;
        self
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    /// Sets the value from the model; it emits nothing, so it cannot echo back as an edit.
    pub fn set_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        let value = value.clamp(*self.range.start(), *self.range.end());
        if value.is_finite() && value != self.value {
            self.value = value;
            self.sync_field(window, cx);
            cx.notify();
        }
    }

    pub fn set_disabled(&mut self, disabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.end_drag(cx);
        }
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
        self.sync_field(window, cx);
        cx.notify();
    }

    pub fn preview(&mut self, state: Interaction, cx: &mut Context<Self>) {
        self.preview = state;
        cx.notify();
    }

    fn field_focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }

    fn apply(&mut self, value: f64, cx: &mut Context<Self>) -> bool {
        if self.disabled || !value.is_finite() {
            return false;
        }
        let value = value.clamp(*self.range.start(), *self.range.end());
        if value == self.value {
            return false;
        }
        self.value = value;
        cx.emit(SliderEvent::Changed(value));
        cx.notify();
        true
    }

    fn sync_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editing = self.field_focused(window, cx);
        self.baseline = self.format.editing(self.value);
        let text: SharedString = if editing {
            self.baseline.clone().into()
        } else {
            self.format.display(self.value)
        };
        if self.field.read(cx).value() != text {
            self.field.update(cx, |field, cx| {
                field.set_value(text, window, cx);
                if editing {
                    field.select_all(window, cx);
                }
            });
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.field.read(cx).value().to_string();
        if text != self.baseline
            && let Some(value) = self.format.parse(&text)
        {
            self.apply(value, cx);
        }
        self.sync_field(window, cx);
    }

    fn revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let baseline: SharedString = self.baseline.clone().into();
        self.field
            .update(cx, |field, cx| field.set_value(baseline, window, cx));
        window.blur(cx);
    }

    fn step(&mut self, direction: f64, window: &mut Window, cx: &mut Context<Self>) {
        if self.apply(self.value + direction * self.format.step, cx) {
            self.sync_field(window, cx);
        }
    }

    fn on_field(
        &mut self,
        field: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Focus => {
                self.sync_field(window, cx);
                field.update(cx, |field, cx| field.select_all(window, cx));
            }
            InputEvent::Blur => self.commit(window, cx),
            InputEvent::PressEnter { .. } => {
                self.commit(window, cx);
                window.blur(cx);
            }
            InputEvent::Change => {}
        }
    }

    fn begin_drag(&mut self, x: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled {
            return;
        }
        window.focus(&self.track_focus, cx);
        if !self.dragging {
            self.dragging = true;
            cx.emit(SliderEvent::DragStarted);
        }
        self.drag_to(x, window, cx);
        cx.notify();
    }

    fn drag_to(&mut self, x: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        let bounds = self.track.get();
        let x = f32::from(x - bounds.left());
        if let Some(value) = value_at(&self.range, x, f32::from(bounds.size.width))
            && self.apply(value, cx)
        {
            self.sync_field(window, cx);
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging {
            self.dragging = false;
            cx.emit(SliderEvent::DragEnded);
            cx.notify();
        }
    }

    fn progress(&self) -> f32 {
        let (lo, hi) = (*self.range.start(), *self.range.end());
        let span = hi - lo;
        if !(span.is_finite() && span > 0.) {
            return 0.;
        }
        (((self.value - lo) / span).clamp(0., 1.)) as f32
    }
}

fn tick(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    x: f32,
    height: f32,
    width: f32,
    colour: Hsla,
) {
    let centre = bounds.center().y;
    let origin = point(bounds.left() + px(x - width / 2.), centre - px(height / 2.));
    window.paint_quad(
        fill(Bounds::new(origin, size(px(width), px(height))), colour).corner_radii(px(width / 2.)),
    );
}

fn paint_markers(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    progress: f32,
    zero: Option<f64>,
    t: &Tokens,
) {
    let w = f32::from(bounds.size.width);
    let lead = (w * 0.34).max(68.).min(w - 40.);
    let trail = lead.max(w - 12.);
    let (minor, zero_colour, marker) =
        (t.text_tertiary, t.text.opacity(0.42), t.text.opacity(0.74));
    match zero {
        Some(zero) => {
            let spacing = ((trail - lead) / (TICKS - 1) as f32).max(8.);
            let zero_x = (zero as f32 * w).clamp(4., w - 4.);
            for direction in [-1., 1.] {
                for n in 1..=32 {
                    let x = zero_x + direction * n as f32 * spacing;
                    if !(lead..=trail).contains(&x) {
                        break;
                    }
                    tick(window, bounds, x, 10., 1., minor);
                }
            }
            tick(window, bounds, zero_x, 14., 1.5, zero_colour);
        }
        None => {
            for n in 0..TICKS {
                let x = lead + n as f32 / (TICKS - 1) as f32 * (trail - lead);
                tick(window, bounds, x, 10., 1., minor);
            }
        }
    }
    let current = (progress * w).clamp(4., w - 4.);
    tick(window, bounds, current, 18., 3., marker);
}

impl Render for InspectorSlider {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let radius = m.slider_radius;
        let disabled = self.disabled;
        let pressed = !disabled
            && (self.dragging
                || self.track_focus.is_focused(window)
                || self.preview == Interaction::Pressed);
        let reveal = !disabled && (pressed || self.hovered || self.preview == Interaction::Hover);
        if reveal && !self.revealed {
            self.reveals += 1;
        }
        self.revealed = reveal;
        let field_ring =
            !disabled && (self.field_focused(window, cx) || self.preview == Interaction::Focused);
        let progress = self.progress();
        let zero = zero_fraction(&self.range);
        let display = self.format.display(self.value);
        let fill_colour = if pressed { t.accent_tint } else { t.fill };
        let entity = cx.entity();
        let track = self.track.clone();
        let dragging = self.dragging;

        let base = canvas(
            move |bounds, _, _| track.set(bounds),
            move |bounds, (), window, _| {
                let w = bounds.size.width * progress;
                let right = (radius - (bounds.size.width - w)).max(px(0.));
                // A fill narrower than the corner would poke out of the track's rounding, so it shrinks to the
                // corner's chord at its own width.
                let cut = (w < radius).then(|| {
                    let r = f32::from(radius);
                    let dx = r - f32::from(w);
                    px(r - (r * r - dx * dx).sqrt())
                });
                let inset = cut.unwrap_or(px(0.));
                let left = if cut.is_some() { w } else { radius };
                let corners = Corners {
                    top_left: left,
                    bottom_left: left,
                    top_right: right,
                    bottom_right: right,
                };
                if w > px(0.) {
                    let origin = point(bounds.left(), bounds.top() + inset);
                    let height = bounds.size.height - inset * 2.;
                    window.paint_quad(
                        fill(Bounds::new(origin, size(w, height)), fill_colour)
                            .corner_radii(corners),
                    );
                }
                if dragging {
                    let moving = entity.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            moving
                                .update(cx, |this, cx| this.drag_to(event.position.x, window, cx));
                        }
                    });
                    let ending = entity.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            ending.update(cx, |this, cx| this.end_drag(cx));
                        }
                    });
                }
            },
        )
        .absolute()
        .size_full();

        let markers = reveal.then(|| {
            let t = t.clone();
            animate(
                div().absolute().inset_0().child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, (), window, _| {
                            paint_markers(window, bounds, progress, zero, &t)
                        },
                    )
                    .size_full(),
                ),
                ("slider-markers", self.reveals),
                SLIDER_MARKERS,
                cx,
                |d, delta| d.opacity(delta),
            )
        });

        let range = Range {
            value: self.value,
            min: *self.range.start(),
            max: *self.range.end(),
            step: self.format.step,
        };
        let track_el = div()
            .id("track")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(radius)
            .bg(t.track)
            .accessible(Role::Slider, self.label.clone())
            .range(range)
            .aria_value(display)
            .aria_description(fl!("controls-slider-hint"))
            .child(base)
            .children(markers)
            .when(self.label_in_track, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .px(LABEL_INSET)
                        .typeset(t.type_scale.value)
                        .text_color(t.text.opacity(0.85))
                        .child(div().truncate().child(self.label.clone())),
                )
            })
            .when(pressed, |d| d.child(focus_ring(&t, radius)))
            .when(!disabled, |d| {
                d.track_focus(&self.track_focus)
                    .tab_index(0)
                    .cursor(CursorStyle::ResizeColumn)
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.hovered = *hovered;
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.begin_drag(event.position.x, window, cx);
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        let direction = match event.keystroke.key.as_str() {
                            "left" => -1.,
                            "right" => 1.,
                            _ => return,
                        };
                        cx.stop_propagation();
                        this.step(direction, window, cx);
                    }))
                    .on_a11y_action(AccessibleAction::Increment, {
                        let this = cx.entity();
                        move |_, window, cx| this.update(cx, |this, cx| this.step(1., window, cx))
                    })
                    .on_a11y_action(AccessibleAction::Decrement, {
                        let this = cx.entity();
                        move |_, window, cx| this.update(cx, |this, cx| this.step(-1., window, cx))
                    })
            });

        let help: SharedString =
            fl!("controls-slider-value-help", title = self.label.to_string()).into();
        let field_el = div()
            .id("value")
            .relative()
            .flex_none()
            .w(m.slider_value_width)
            .h_full()
            .rounded(radius)
            .bg(t.value_box)
            .text_color(t.text.opacity(0.9))
            .tip(help)
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                cx.stop_propagation();
                this.revert(window, cx);
            }))
            .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                cx.stop_propagation();
                this.commit(window, cx);
                this.step(1., window, cx);
            }))
            .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                cx.stop_propagation();
                this.commit(window, cx);
                this.step(-1., window, cx);
            }))
            .child(
                Input::new(&self.field)
                    .appearance(false)
                    .disabled(disabled)
                    .aria_label(fl!("controls-slider-value", title = self.label.to_string()))
                    .h_full()
                    .px(px(8.))
                    .text_center()
                    .typeset(t.type_scale.value),
            )
            .when(field_ring, |d| d.child(focus_ring(&t, radius)));

        div()
            .flex()
            .w_full()
            .h(m.slider_height)
            .gap(GAP)
            .when(disabled, |d| d.opacity(0.48))
            .child(track_el)
            .child(field_el)
    }
}
