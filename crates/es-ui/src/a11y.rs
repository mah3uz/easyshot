use gpui::{SharedString, StatefulInteractiveElement};

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

impl<E: StatefulInteractiveElement> Accessible for E {}
