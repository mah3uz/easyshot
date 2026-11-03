pub(crate) mod menus;
pub(crate) mod style_bar;
mod text_style;
pub mod tools;

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use es_anno::{GestureId, StylePatch};
use es_model::{RedactionKind, Tool};
use es_ui::{
    components::{EditorButton, InspectorSlider, Popover, SliderEvent, ValueFormat},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    Anchor, AnyElement, App, AppContext, Bounds, Context, Div, Entity, EventEmitter, IntoElement,
    ParentElement, Pixels, Render, Styled, Subscription, Window, canvas, div,
    prelude::FluentBuilder, px,
};

use crate::session::{
    EditorSession,
    doc::{DocEvent, DocModel},
};
use crate::strings::fl;
use menus::{Pick, choice_menu, menu_trigger};

const GAP: Pixels = px(4.);
const DIVIDER_HEIGHT: Pixels = px(24.);
const REDACT_WIDTH: Pixels = px(340.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipAxis {
    Horizontal,
    Vertical,
}

/// What the row asks its window to do; tools and styles go straight to the engine instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRowEvent {
    Crop,
    Rotate(Rotation),
    Flip(FlipAxis),
    SmartRedact(RedactionKind),
    ToggleInspector,
}

/// A control whose last drawn bounds the row remembers, for anchoring and for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Crop,
    Rotate,
    RotateMenu,
    Tool(Tool),
    SmartRedact,
    RedactPixelate,
    RedactBlur,
    Palette,
    ColorCapsule,
    Swatches,
    Stroke,
    TextStyle,
    TextSwatches,
    Strength,
    Delete,
    Inspector,
}

type Drawn = Rc<RefCell<HashMap<Control, Bounds<Pixels>>>>;

pub struct ToolRow {
    doc: Entity<DocModel>,
    cropping: bool,
    inspector_open: bool,
    redact_open: bool,
    redact_busy: bool,
    ocr_ready: bool,
    strength: Option<Entity<InspectorSlider>>,
    gestures: u64,
    dragging: Option<GestureId>,
    drawn: Drawn,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ToolRowEvent> for ToolRow {}

impl ToolRow {
    pub fn new(session: &EditorSession, cx: &mut Context<Self>) -> Self {
        let doc = session.doc.clone();
        let on_doc = cx.subscribe(&doc, |_, _, _: &DocEvent, cx| cx.notify());
        Self {
            doc,
            cropping: false,
            inspector_open: true,
            redact_open: false,
            redact_busy: false,
            ocr_ready: true,
            strength: None,
            gestures: 0,
            dragging: None,
            drawn: Rc::default(),
            _subscriptions: vec![on_doc],
        }
    }

    /// Built on first draw, since the slider needs a window; kept in step with the engine's density.
    fn strength(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<InspectorSlider> {
        let density = self.doc.read(cx).editor().current_style().redaction_density;
        if let Some(slider) = &self.strength {
            if slider.read(cx).value() != density.clamp(0.15, 1.0) {
                slider.update(cx, |s, cx| s.set_value(density, window, cx));
            }
            return slider.clone();
        }
        let slider = cx.new(|cx| {
            InspectorSlider::new(
                fl!("editor-redact-strength"),
                density,
                0.15..=1.0,
                ValueFormat::percent(),
                window,
                cx,
            )
        });
        let on_strength = cx.subscribe(&slider, |this, _, event: &SliderEvent, cx| match event {
            SliderEvent::DragStarted => this.dragging = Some(this.fresh_gesture()),
            SliderEvent::Changed(value) => {
                let gesture = this.dragging;
                this.apply_style(StylePatch::RedactionDensity(*value), gesture, cx);
            }
            SliderEvent::DragEnded => this.dragging = None,
        });
        self._subscriptions.push(on_strength);
        self.strength = Some(slider.clone());
        slider
    }

    /// While cropping only the crop toggle stays live.
    pub fn set_cropping(&mut self, cropping: bool, cx: &mut Context<Self>) {
        self.cropping = cropping;
        cx.notify();
    }

    pub fn set_redact_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.redact_busy = busy;
        cx.notify();
    }

    pub fn set_ocr_ready(&mut self, ready: bool, cx: &mut Context<Self>) {
        self.ocr_ready = ready;
        cx.notify();
    }

    pub fn set_inspector_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.inspector_open = open;
        cx.notify();
    }

    /// Where a control was last drawn, in window coordinates; `None` when it is not shown.
    pub fn control_bounds(&self, control: Control) -> Option<Bounds<Pixels>> {
        self.drawn.borrow().get(&control).copied()
    }

    /// Picking the armed tool again returns to Select.
    pub fn pick_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        self.doc
            .update(cx, |d, cx| d.apply(cx, |e| e.select_tool(tool)));
    }

    /// Sets the style for new shapes and restyles the selection as one undo step; calls sharing a gesture make one.
    pub fn apply_style(
        &mut self,
        patch: StylePatch,
        gesture: Option<GestureId>,
        cx: &mut Context<Self>,
    ) {
        let gesture = gesture.unwrap_or_else(|| self.fresh_gesture());
        self.doc
            .update(cx, |d, cx| d.apply(cx, |e| e.apply_style(patch, gesture)));
        cx.notify();
    }

    fn fresh_gesture(&mut self) -> GestureId {
        self.gestures += 1;
        GestureId(self.gestures)
    }

    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        self.doc
            .update(cx, |d, cx| d.apply(cx, |e| e.delete_selected()));
    }

    fn track(&self, control: Control, child: impl IntoElement) -> Div {
        track(&self.drawn, control, child)
    }

    fn emitter<T: 'static>(
        &self,
        cx: &mut Context<Self>,
        event: impl Fn(T) -> ToolRowEvent + 'static,
    ) -> Pick<T> {
        let row = cx.entity().downgrade();
        Rc::new(move |value, _, cx| {
            let _ = row.update(cx, |_, cx| cx.emit(event(value)));
        })
    }

    fn rotate_group(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = cx.entity().downgrade();
        let rotate = EditorButton::new("rotate")
            .icon(IconName::RotateCcw)
            .accessible_label(fl!("editor-rotate-left"))
            .tooltip(fl!("editor-rotate-help"))
            .padding_x(tokens(cx).metrics.tool_button_padding_x)
            .disabled(self.cropping)
            .on_click(move |_, _, cx| {
                let _ = row.update(cx, |_, cx| cx.emit(ToolRowEvent::Rotate(Rotation::Left)));
            });
        let choices = vec![
            (
                ToolRowEvent::Rotate(Rotation::Left),
                fl!("editor-rotate-left"),
                Some(IconName::RotateCcw),
            ),
            (
                ToolRowEvent::Rotate(Rotation::Right),
                fl!("editor-rotate-right"),
                Some(IconName::RotateCw),
            ),
            (
                ToolRowEvent::Flip(FlipAxis::Horizontal),
                fl!("editor-flip-horizontal"),
                Some(IconName::FlipHorizontal2),
            ),
            (
                ToolRowEvent::Flip(FlipAxis::Vertical),
                fl!("editor-flip-vertical"),
                Some(IconName::FlipVertical2),
            ),
        ];
        let trigger = menu_trigger(
            "rotate-menu",
            fl!("editor-rotate-menu"),
            None,
            None,
            self.cropping,
            cx,
        )
        .px(px(3.));
        let menu = choice_menu(trigger, choices, None, self.emitter(cx, |event| event));
        div()
            .flex()
            .items_center()
            .child(self.track(Control::Rotate, rotate))
            .child(self.track(Control::RotateMenu, menu))
    }

    fn smart_redaction(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = cx.entity().downgrade();
        let drawn = self.drawn.clone();
        let unavailable = !self.ocr_ready;
        let blocked = self.redact_busy || unavailable;
        let trigger = EditorButton::new("redact-button")
            .icon(IconName::EyeOff)
            .accessible_label(fl!("editor-redact"))
            .tooltip(if unavailable {
                fl!("editor-redact-unavailable")
            } else {
                fl!("editor-redact-help")
            })
            .padding_x(tokens(cx).metrics.tool_button_padding_x)
            .selected(self.redact_open)
            .disabled(self.cropping || blocked);
        let on_open = row.clone();
        Popover::new("redact", fl!("editor-redact"))
            .anchor(Anchor::TopLeft)
            .width(REDACT_WIDTH)
            .open(self.redact_open)
            .on_open_change(move |open, _, cx| {
                let _ = on_open.update(cx, |r, cx| {
                    r.redact_open = open;
                    cx.notify();
                });
            })
            .trigger(trigger)
            .content(move |_, cx| {
                let t = tokens(cx).clone();
                let choice = |control, id: &'static str, icon, label: String, kind| {
                    let row = row.clone();
                    track(
                        &drawn,
                        control,
                        EditorButton::new(id)
                            .icon(icon)
                            .label(label)
                            .kind(es_ui::components::ButtonKind::Bordered)
                            .disabled(blocked)
                            .on_click(move |_, _, cx| {
                                let _ = row.update(cx, |r, cx| {
                                    r.redact_open = false;
                                    cx.emit(ToolRowEvent::SmartRedact(kind));
                                    cx.notify();
                                });
                            }),
                    )
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .p(px(8.))
                    .child(
                        div()
                            .typeset(t.type_scale.title)
                            .child(fl!("editor-redact")),
                    )
                    .child(
                        div()
                            .text_color(t.text_secondary)
                            .typeset(t.type_scale.label)
                            .child(fl!("editor-redact-body")),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(choice(
                                Control::RedactPixelate,
                                "redact-pixelate",
                                IconName::Pixelate,
                                fl!("editor-redact-pixelate"),
                                RedactionKind::Pixelate,
                            ))
                            .child(choice(
                                Control::RedactBlur,
                                "redact-blur",
                                IconName::Droplet,
                                fl!("editor-redact-blur"),
                                RedactionKind::Blur,
                            )),
                    )
            })
    }
}

fn track(drawn: &Drawn, control: Control, child: impl IntoElement) -> Div {
    let drawn = drawn.clone();
    div().relative().flex_none().child(child).child(
        canvas(
            move |bounds, _, _| {
                drawn.borrow_mut().insert(control, bounds);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

fn divider(cx: &App) -> Div {
    div()
        .flex_none()
        .w(tokens(cx).metrics.hairline)
        .h(DIVIDER_HEIGHT)
        .mx(px(4.))
        .bg(tokens(cx).separator)
}

impl Render for ToolRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn.borrow_mut().clear();
        let t = tokens(cx).clone();
        let (tool, straight) = {
            let editor = self.doc.read(cx).editor();
            (editor.tool(), editor.highlighter_straight())
        };
        let padding = t.metrics.tool_button_padding_x;
        let row = cx.entity().downgrade();

        let crop = {
            let row = row.clone();
            EditorButton::new("crop")
                .icon(IconName::Crop)
                .accessible_label(fl!("editor-crop"))
                .tooltip(fl!("editor-crop-help"))
                .padding_x(padding)
                .selected(self.cropping)
                .toggle(true)
                .on_click(move |_, _, cx| {
                    let _ = row.update(cx, |_, cx| cx.emit(ToolRowEvent::Crop));
                })
        };
        let tools: Vec<AnyElement> = Tool::ALL
            .iter()
            .enumerate()
            .map(|(i, &each)| {
                let row = row.clone();
                let (icon, tooltip) = if each == Tool::Highlighter {
                    let icon = if straight {
                        IconName::HighlighterLine
                    } else {
                        IconName::Highlighter
                    };
                    (icon, tools::highlighter_tooltip(straight))
                } else {
                    (tools::icon(each), tools::tooltip(each))
                };
                let button = EditorButton::new(("tool", i))
                    .icon(icon)
                    .accessible_label(tools::title(each))
                    .tooltip(tooltip)
                    .padding_x(padding)
                    .selected(each == tool)
                    .toggle(true)
                    .disabled(self.cropping)
                    .on_click(move |_, _, cx| {
                        let _ = row.update(cx, |r, cx| r.pick_tool(each, cx));
                    });
                self.track(Control::Tool(each), button).into_any_element()
            })
            .collect();
        let style_bar = (!self.cropping)
            .then(|| style_bar::render(self, window, cx))
            .flatten();
        let inspector = {
            let row = row.clone();
            let tip = if self.inspector_open {
                fl!("editor-inspector-hide")
            } else {
                fl!("editor-inspector-show")
            };
            EditorButton::new("inspector")
                .icon(IconName::PanelRight)
                .accessible_label(fl!("editor-inspector-toggle-a11y"))
                .tooltip(tip)
                .padding_x(padding)
                .on_click(move |_, _, cx| {
                    let _ = row.update(cx, |_, cx| cx.emit(ToolRowEvent::ToggleInspector));
                })
        };
        div()
            .flex()
            .flex_1()
            .items_center()
            .h_full()
            .gap(GAP)
            .text_color(t.text)
            .child(self.track(Control::Crop, crop))
            .child(self.rotate_group(cx))
            .child(divider(cx))
            .children(tools)
            .child(self.track(Control::SmartRedact, self.smart_redaction(cx)))
            .child(divider(cx))
            .when_some(style_bar, |d, bar| d.child(bar))
            .child(div().flex_1())
            .child(self.track(Control::Inspector, inspector))
    }
}
