use crate::swatch::Swatch;
use es_geom::vec::PVec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashStyle {
    Draw,
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillStyle {
    #[default]
    None,
    Solid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeoKind {
    #[default]
    Rectangle,
    Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrowhead {
    #[default]
    None,
    Arrow,
    Triangle,
    Square,
    Dot,
    Diamond,
    Inverted,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    #[default]
    Pro,
    Compact,
    Rounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionKind {
    #[default]
    Blur,
    Pixelate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeoProps {
    pub geo: GeoKind,
    pub w: f64,
    pub h: f64,
    pub swatch: Swatch,
    pub stroke_width: f64,
    pub fill: FillStyle,
    pub dash: DashStyle,
    /// Page pixels; rectangles only. Geometry treats 0.5 or less as sharp.
    pub corner_radius: f64,
}

impl Default for GeoProps {
    fn default() -> Self {
        Self {
            geo: GeoKind::Rectangle,
            w: 100.0,
            h: 100.0,
            swatch: Swatch::default(),
            stroke_width: 8.0,
            fill: FillStyle::None,
            dash: DashStyle::Solid,
            corner_radius: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawProps {
    /// Local space; `z` is pen pressure (0.5 for a mouse).
    pub points: Vec<PVec>,
    pub is_complete: bool,
    pub is_pen: bool,
    pub is_closed: bool,
    pub swatch: Swatch,
    pub stroke_width: f64,
}

impl Default for DrawProps {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            is_complete: false,
            is_pen: false,
            is_closed: false,
            swatch: Swatch::default(),
            stroke_width: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArrowProps {
    pub start: PVec,
    pub end: PVec,
    /// Signed distance the arrow bows from the chord.
    pub bend: f64,
    pub arrowhead_start: Arrowhead,
    pub arrowhead_end: Arrowhead,
    pub swatch: Swatch,
    pub stroke_width: f64,
    pub dash: DashStyle,
}

impl Default for ArrowProps {
    fn default() -> Self {
        Self {
            start: PVec::new(0.0, 0.0),
            end: PVec::new(100.0, 0.0),
            bend: 0.0,
            arrowhead_start: Arrowhead::None,
            arrowhead_end: Arrowhead::Arrow,
            swatch: Swatch::default(),
            stroke_width: 8.0,
            dash: DashStyle::Solid,
        }
    }
}

pub const DEFAULT_TEXT_SIZE: f64 = 36.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextProps {
    pub text: String,
    pub swatch: Swatch,
    pub font_size: f64,
    pub font_family: FontFamily,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_underline: bool,
    pub align: TextAlign,
    /// Wrap width; meaningful only when `auto_size` is off.
    pub w: f64,
    pub auto_size: bool,
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            text: String::new(),
            swatch: Swatch::default(),
            font_size: DEFAULT_TEXT_SIZE,
            font_family: FontFamily::Pro,
            is_bold: true,
            is_italic: false,
            is_underline: false,
            align: TextAlign::Start,
            w: 16.0,
            auto_size: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RedactionProps {
    pub kind: RedactionKind,
    pub w: f64,
    pub h: f64,
    /// 0..=1; drives the blur radius and the pixel block size.
    pub density: f64,
}

impl Default for RedactionProps {
    fn default() -> Self {
        Self {
            kind: RedactionKind::Blur,
            w: 100.0,
            h: 100.0,
            density: 0.55,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightProps {
    pub w: f64,
    pub h: f64,
}

impl Default for HighlightProps {
    fn default() -> Self {
        Self { w: 100.0, h: 100.0 }
    }
}

/// The highlighter paints this many times the width the stroke slider names.
pub const HIGHLIGHTER_WIDTH_FACTOR: f64 = 3.0;

/// Straight mode keeps two points; freehand keeps every sampled point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlighterProps {
    pub points: Vec<PVec>,
    pub swatch: Swatch,
    pub stroke_width: f64,
}

impl Default for HighlighterProps {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            swatch: Swatch::default(),
            stroke_width: 24.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NumberedProps {
    pub value: u32,
    pub diameter: f64,
    pub swatch: Swatch,
}

impl Default for NumberedProps {
    fn default() -> Self {
        Self {
            value: 1,
            diameter: 64.0,
            swatch: Swatch::default(),
        }
    }
}
