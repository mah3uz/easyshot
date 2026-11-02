use gpui::{App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window};

use crate::{
    components::button::{ClickHandler, EditorButton, Interaction},
    fl,
    icons::IconName,
    theme::tokens,
};

#[derive(IntoElement)]
pub struct ToolButton {
    id: ElementId,
    icon: IconName,
    title: SharedString,
    shortcut: Option<SharedString>,
    selected: bool,
    disabled: bool,
    preview: Interaction,
    on_click: Option<ClickHandler>,
}

impl ToolButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            title: title.into(),
            shortcut: None,
            selected: false,
            disabled: false,
            preview: Interaction::Rest,
            on_click: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(std::rc::Rc::new(handler));
        self
    }
}

impl RenderOnce for ToolButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tooltip: SharedString = match &self.shortcut {
            Some(shortcut) => fl!(
                "ui-shortcut-help",
                title = self.title.to_string(),
                shortcut = shortcut.to_string()
            )
            .into(),
            None => self.title.clone(),
        };
        let mut button = EditorButton::new(self.id)
            .icon(self.icon)
            .accessible_label(self.title)
            .tooltip(tooltip)
            .padding_x(tokens(cx).metrics.tool_button_padding_x)
            .selected(self.selected)
            .toggle(true)
            .disabled(self.disabled)
            .preview(self.preview);
        if let Some(on_click) = self.on_click {
            button = button.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        button
    }
}
