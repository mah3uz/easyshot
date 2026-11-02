use es_protocol::{Modifiers, Trigger};
use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, Keystroke, ModifiersChangedEvent, MouseButton, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, prelude::FluentBuilder, px,
};

use crate::{
    a11y::{Accessible, Role},
    components::{
        button::{Interaction, focus_halo},
        icon_button::IconButton,
    },
    fl,
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};

const MODIFIER_KEYS: [&str; 5] = ["shift", "control", "alt", "platform", "function"];

/// The field never adopts what it records: the caller may refuse a trigger, then answers with `set_trigger`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutFieldEvent {
    Recorded(Trigger),
    Cleared,
    RecordingChanged(bool),
}

pub fn modifiers_of(keystroke: &gpui::Modifiers) -> Modifiers {
    let mut out = Modifiers::empty();
    out.set(Modifiers::SUPER, keystroke.platform);
    out.set(Modifiers::CTRL, keystroke.control);
    out.set(Modifiers::ALT, keystroke.alt);
    out.set(Modifiers::SHIFT, keystroke.shift);
    out
}

pub fn trigger_for(keystroke: &Keystroke) -> Option<Trigger> {
    let key = keystroke.key.as_str();
    let key = match key {
        "enter" => "Return".to_owned(),
        "pageup" => "PageUp".to_owned(),
        "pagedown" => "PageDown".to_owned(),
        "-" => "Minus".to_owned(),
        "=" => "Equal".to_owned(),
        "+" => "Plus".to_owned(),
        k if k.len() > 1 && k.starts_with('f') && k[1..].bytes().all(|b| b.is_ascii_digit()) => {
            k.to_ascii_uppercase()
        }
        k => k.to_owned(),
    };
    let mut trigger = Trigger::parse(&key).ok()?;
    trigger.modifiers = modifiers_of(&keystroke.modifiers);
    Some(trigger)
}

fn held_label(modifiers: Modifiers, spoken: bool) -> Option<String> {
    if modifiers.is_empty() {
        return None;
    }
    let partial = Trigger {
        modifiers,
        key: "A".to_owned(),
    };
    let (text, tail) = if spoken {
        (partial.spoken(), ", A")
    } else {
        (partial.label(), "+A")
    };
    text.strip_suffix(tail).map(str::to_owned)
}

pub struct ShortcutField {
    label: SharedString,
    trigger: Option<Trigger>,
    clears_on_delete: bool,
    recording: bool,
    held_modifiers: Modifiers,
    held_key: Option<String>,
    focus: FocusHandle,
    intercept: Option<Subscription>,
    preview: Interaction,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<ShortcutFieldEvent> for ShortcutField {}

impl Focusable for ShortcutField {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ShortcutField {
    pub fn new(
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let _subscriptions = [
            cx.on_focus(&focus, window, Self::begin_recording),
            cx.on_blur(&focus, window, |this, _, cx| this.cancel_recording(cx)),
            cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.cancel_recording(cx);
                }
            }),
        ];
        Self {
            label: label.into(),
            trigger: None,
            clears_on_delete: false,
            recording: false,
            held_modifiers: Modifiers::empty(),
            held_key: None,
            focus,
            intercept: None,
            preview: Interaction::Rest,
            _subscriptions,
        }
    }

    pub fn trigger(&self) -> Option<&Trigger> {
        self.trigger.as_ref()
    }

    pub fn set_trigger(&mut self, trigger: Option<Trigger>, cx: &mut Context<Self>) {
        self.trigger = trigger;
        cx.notify();
    }

    /// Global actions clear on a bare Delete; editor actions can bind it and use the clear button instead.
    pub fn set_clears_on_delete(&mut self, clears: bool, cx: &mut Context<Self>) {
        self.clears_on_delete = clears;
        cx.notify();
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn preview(&mut self, state: Interaction, held: Option<Modifiers>, cx: &mut Context<Self>) {
        self.preview = state;
        self.recording = held.is_some();
        self.held_modifiers = held.unwrap_or(Modifiers::empty());
        cx.notify();
    }

    /// Display text, and the value read to assistive technology.
    pub fn text(&self) -> (SharedString, SharedString) {
        if self.recording {
            let press: SharedString = fl!("controls-shortcut-press").into();
            return match (
                held_label(self.held_modifiers, false),
                held_label(self.held_modifiers, true),
            ) {
                (Some(shown), Some(spoken)) => (shown.into(), spoken.into()),
                _ => (press.clone(), press),
            };
        }
        match &self.trigger {
            Some(trigger) => (trigger.label().into(), trigger.spoken().into()),
            None => (
                SharedString::default(),
                fl!("ui-shortcut-unassigned").into(),
            ),
        }
    }

    pub fn begin_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording {
            return;
        }
        self.recording = true;
        self.held_key = None;
        self.held_modifiers = modifiers_of(&window.modifiers());
        let this = cx.weak_entity();
        let focus = self.focus.clone();
        // A window-wide hook, so keys the app has bound elsewhere are recorded instead of acted on.
        self.intercept = Some(cx.intercept_keystrokes(move |event, window, cx| {
            if !focus.is_focused(window) {
                return;
            }
            let consumed = this
                .update(cx, |this, cx| this.key(&event.keystroke, cx))
                .unwrap_or(false);
            if consumed {
                cx.stop_propagation();
            }
        }));
        cx.emit(ShortcutFieldEvent::RecordingChanged(true));
        cx.notify();
    }

    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        if !self.recording {
            return;
        }
        self.recording = false;
        self.intercept = None;
        self.held_modifiers = Modifiers::empty();
        cx.emit(ShortcutFieldEvent::RecordingChanged(false));
        cx.notify();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        cx.emit(ShortcutFieldEvent::Cleared);
        self.cancel_recording(cx);
    }

    fn key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let key = keystroke.key.as_str();
        if !self.recording || MODIFIER_KEYS.contains(&key) {
            return false;
        }
        if self.held_key.as_deref() == Some(key) {
            return true;
        }
        self.held_key = Some(key.to_owned());
        let bare = modifiers_of(&keystroke.modifiers).is_empty();
        match key {
            "escape" => self.cancel_recording(cx),
            "tab" => {
                self.cancel_recording(cx);
                return false;
            }
            "backspace" | "delete" if bare && self.clears_on_delete => self.clear(cx),
            _ => {
                if let Some(trigger) = trigger_for(keystroke) {
                    cx.emit(ShortcutFieldEvent::Recorded(trigger));
                    self.cancel_recording(cx);
                }
            }
        }
        true
    }
}

impl Render for ShortcutField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let (shown, spoken) = self.text();
        let ring = self.recording
            || self.preview == Interaction::Focused
            || (self.focus.is_focused(window) && window.last_input_was_keyboard());
        let empty = shown.is_empty();
        let help = if self.clears_on_delete {
            fl!("controls-shortcut-help-delete")
        } else {
            fl!("controls-shortcut-help")
        };
        let clearable = self.trigger.is_some() && !self.recording;
        let fill = match self.preview {
            Interaction::Hover => t.hover_fill,
            _ => t.bordered_fill,
        };
        let hover_fill = t.hover_fill;
        div()
            .id("shortcut-field")
            .relative()
            .flex()
            .items_center()
            .min_w(px(124.))
            .h(m.control_height)
            .px(px(6.))
            .rounded(m.field_radius)
            .border(m.hairline)
            .border_color(t.border)
            .bg(fill)
            .typeset(t.type_scale.value)
            .accessible(Role::Button, self.label.clone())
            .aria_value(spoken)
            .aria_description(help)
            .track_focus(&self.focus)
            .tab_index(0)
            .when(self.preview == Interaction::Rest && !self.recording, |d| {
                d.hover(move |s| s.bg(hover_fill))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    this.begin_recording(window, cx);
                }),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.cancel_recording(cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if !this.recording && !event.is_held && matches!(key, "space" | "enter") {
                    cx.stop_propagation();
                    this.begin_recording(window, cx);
                    this.held_key = Some(key.to_owned());
                }
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                if this.held_key.as_deref() == Some(event.keystroke.key.as_str()) {
                    this.held_key = None;
                }
            }))
            .on_modifiers_changed(cx.listener(|this, event: &ModifiersChangedEvent, _, cx| {
                if this.recording {
                    this.held_modifiers = modifiers_of(&event.modifiers);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .justify_center()
                    .truncate()
                    .text_color(if self.recording {
                        t.text_secondary
                    } else if empty {
                        t.text_tertiary
                    } else {
                        t.text
                    })
                    .child(if empty {
                        SharedString::from(fl!("controls-shortcut-placeholder"))
                    } else {
                        shown
                    }),
            )
            .when(clearable, |d| {
                d.child(
                    div()
                        .absolute()
                        .right(px(3.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            IconButton::new(
                                "clear",
                                IconName::CircleX,
                                fl!("controls-shortcut-clear"),
                            )
                            .size(px(16.), px(12.))
                            .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                        ),
                )
            })
            .when(ring, |d| d.child(focus_halo(&t, m.field_radius)))
    }
}
