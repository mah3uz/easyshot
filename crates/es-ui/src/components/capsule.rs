use gpui::{
    AnyElement, App, ElementId, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, Styled, Window, div, px,
};

use crate::{
    a11y::{Accessible, Role},
    theme::tokens,
};

/// A pill that groups a few related controls, such as undo and redo, into one floating unit.
#[derive(IntoElement)]
pub struct Capsule {
    id: ElementId,
    label: SharedString,
    children: Vec<AnyElement>,
}

impl Capsule {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
        }
    }
}

impl ParentElement for Capsule {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Capsule {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded_full()
            .border(t.metrics.hairline)
            .border_color(t.border)
            .bg(t.window)
            .accessible(Role::Group, self.label)
            .children(self.children)
    }
}
