mod engine;
mod fonts;
mod layout;
mod lru;

pub use engine::{LINE_HEIGHT_MULTIPLE, MIN_WIDTH, TextEngine, line_height};
pub use fonts::{BUNDLED, FontSet};
pub use layout::{LineBox, Outline, PlacedGlyph, TextLayout};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    #[default]
    Pro,
    Compact,
    Rounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    #[default]
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: TextAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::Pro,
            size: 48.0,
            bold: true,
            italic: false,
            underline: false,
            align: TextAlign::Start,
        }
    }
}
