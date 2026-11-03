use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Select,
    #[default]
    Rectangle,
    FilledRectangle,
    Ellipse,
    Line,
    Arrow,
    Freehand,
    NumberedCircle,
    Text,
    Highlight,
    Highlighter,
    Pixelate,
    Blur,
}

impl Tool {
    pub const ALL: [Tool; 13] = [
        Tool::Select,
        Tool::Rectangle,
        Tool::FilledRectangle,
        Tool::Ellipse,
        Tool::Line,
        Tool::Arrow,
        Tool::Freehand,
        Tool::NumberedCircle,
        Tool::Text,
        Tool::Highlight,
        Tool::Highlighter,
        Tool::Pixelate,
        Tool::Blur,
    ];

    pub fn creates_annotation(self) -> bool {
        self != Tool::Select
    }

    pub fn supports_color(self) -> bool {
        !matches!(
            self,
            Tool::Select | Tool::Highlight | Tool::Pixelate | Tool::Blur
        )
    }

    pub fn supports_stroke_width(self) -> bool {
        matches!(
            self,
            Tool::Rectangle
                | Tool::Ellipse
                | Tool::Line
                | Tool::Arrow
                | Tool::Freehand
                | Tool::Highlighter
        )
    }

    pub fn is_redaction(self) -> bool {
        matches!(self, Tool::Pixelate | Tool::Blur)
    }

    pub fn supports_aspect_lock(self) -> bool {
        matches!(
            self,
            Tool::Rectangle | Tool::FilledRectangle | Tool::Ellipse | Tool::Highlight
        )
    }

    pub fn is_filled_shape(self) -> bool {
        self == Tool::FilledRectangle
    }

    pub fn uses_endpoints(self) -> bool {
        matches!(self, Tool::Line | Tool::Arrow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_table_matches_the_tool_rules() {
        let color: Vec<_> = Tool::ALL.iter().filter(|t| t.supports_color()).collect();
        assert_eq!(color.len(), 9);
        assert!(!Tool::Highlight.supports_color());
        assert!(Tool::Text.supports_color() && !Tool::Text.supports_stroke_width());
        assert!(
            Tool::NumberedCircle.supports_color() && !Tool::NumberedCircle.supports_stroke_width()
        );
        assert!(
            Tool::FilledRectangle.supports_color()
                && !Tool::FilledRectangle.supports_stroke_width()
        );
        assert!(Tool::Blur.is_redaction() && Tool::Pixelate.is_redaction());
        assert!(!Tool::Select.creates_annotation());
        assert_eq!(Tool::default(), Tool::Rectangle);
    }
}
