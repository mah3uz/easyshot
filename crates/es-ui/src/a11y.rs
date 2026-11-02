use gpui::{App, KeyBinding, SharedString, StatefulInteractiveElement, actions};

pub use gpui::{Role, Toggled};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// The element also needs an `id` to reach the accessibility tree.
pub trait Accessible: StatefulInteractiveElement + Sized {
    fn accessible(self, role: Role, label: impl Into<SharedString>) -> Self {
        self.role(role).aria_label(label)
    }

    fn range(self, range: Range) -> Self {
        self.aria_numeric_value(range.value)
            .aria_min_numeric_value(range.min)
            .aria_max_numeric_value(range.max)
            .aria_numeric_value_step(range.step)
    }

    fn toggled(self, on: bool) -> Self {
        self.aria_toggled(if on { Toggled::True } else { Toggled::False })
    }
}

actions!(es_ui, [FocusNext, FocusPrev]);

/// Context-free, so a deeper context's own Tab binding still wins.
pub fn bind_focus_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrev, None),
    ]);
    // A global action handler never sees its window; a keystroke observer does, and runs only once the action is handled.
    cx.on_action(|_: &FocusNext, _| {});
    cx.on_action(|_: &FocusPrev, _| {});
    cx.observe_keystrokes(|event, window, cx| {
        let Some(action) = event.action.as_deref().map(|a| a.as_any()) else {
            return;
        };
        if action.is::<FocusNext>() {
            window.focus_next(cx);
        } else if action.is::<FocusPrev>() {
            window.focus_prev(cx);
        }
    })
    .detach();
}

impl<E: StatefulInteractiveElement> Accessible for E {}
