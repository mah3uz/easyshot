use es_model::Tool;
use es_protocol::{Trigger, entry_by_id};
use es_ui::icons::IconName;

use crate::strings::fl;

pub fn icon(tool: Tool) -> IconName {
    match tool {
        Tool::Select => IconName::Pointer,
        Tool::Rectangle => IconName::RectangleHorizontal,
        Tool::FilledRectangle => IconName::Square,
        Tool::Ellipse => IconName::Circle,
        Tool::Line => IconName::Slash,
        Tool::Arrow => IconName::ArrowUpRight,
        Tool::Freehand => IconName::PencilLine,
        Tool::NumberedCircle => IconName::Counter,
        Tool::Text => IconName::ALargeSmall,
        Tool::Highlight => IconName::Highlight,
        Tool::Pixelate => IconName::Pixelate,
        Tool::Blur => IconName::Droplet,
    }
}

pub fn title(tool: Tool) -> String {
    match tool {
        Tool::Select => fl!("editor-tool-select"),
        Tool::Rectangle => fl!("editor-tool-rectangle"),
        Tool::FilledRectangle => fl!("editor-tool-solid-rectangle"),
        Tool::Ellipse => fl!("editor-tool-circle"),
        Tool::Line => fl!("editor-tool-line"),
        Tool::Arrow => fl!("editor-tool-arrow"),
        Tool::Freehand => fl!("editor-tool-freehand"),
        Tool::NumberedCircle => fl!("editor-tool-numbered-circle"),
        Tool::Text => fl!("editor-tool-text"),
        Tool::Highlight => fl!("editor-tool-highlight"),
        Tool::Pixelate => fl!("editor-tool-pixelate"),
        Tool::Blur => fl!("editor-tool-blur"),
    }
}

fn shortcut_id(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "image-select",
        Tool::Rectangle => "image-rectangle",
        Tool::FilledRectangle => "image-filled-rectangle",
        Tool::Ellipse => "image-ellipse",
        Tool::Line => "image-line",
        Tool::Arrow => "image-arrow",
        Tool::Freehand => "image-freehand",
        Tool::NumberedCircle => "image-counter",
        Tool::Text => "image-text",
        Tool::Highlight => "image-highlight",
        Tool::Pixelate => "image-pixelate",
        Tool::Blur => "image-blur",
    }
}

/// The catalogue's default trigger, as the key cap shows it.
pub fn shortcut_label(id: &str) -> Option<String> {
    let trigger = entry_by_id(id)?.default_trigger?;
    Trigger::parse(trigger).ok().map(|t| t.label())
}

/// "Title (key)"; a tool with no key shows its title, and Highlight explains itself instead.
pub fn tooltip(tool: Tool) -> String {
    if tool == Tool::Highlight {
        return fl!("editor-tool-highlight-help");
    }
    match shortcut_label(shortcut_id(tool)) {
        Some(shortcut) => fl!(
            "editor-shortcut-help",
            title = title(tool),
            shortcut = shortcut
        ),
        None => title(tool),
    }
}
