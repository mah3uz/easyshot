use es_model::{
    AnnoShape, ArrowProps, Arrowhead, DashStyle, FontFamily, HIGHLIGHTER_WIDTH_FACTOR, ShapeKind,
    StylePreset, Swatch, TextAlign, TextBorder, Tool,
};

pub const MIN_FONT_SIZE: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CurrentStyle {
    pub swatch: Swatch,
    /// Slider units; the page width depends on the image size.
    pub stroke_width: f64,
    pub redaction_density: f64,
    pub font_family: FontFamily,
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub align: TextAlign,
    pub text_fill: Option<Swatch>,
    pub text_border: Option<TextBorder>,
    pub arrowhead_start: Arrowhead,
    pub arrowhead_end: Arrowhead,
    pub dash: DashStyle,
}

impl Default for CurrentStyle {
    fn default() -> Self {
        Self::from(&StylePreset::default())
    }
}

impl From<&StylePreset> for CurrentStyle {
    fn from(p: &StylePreset) -> Self {
        Self {
            swatch: p.swatch.clone(),
            stroke_width: p.stroke_width,
            redaction_density: p.redaction_density,
            font_family: p.text_font_family.clone(),
            font_size: p.text_font_size,
            bold: p.text_bold,
            italic: p.text_italic,
            underline: p.text_underline,
            align: p.text_align,
            text_fill: p.text_fill.clone(),
            text_border: p.text_border.clone(),
            arrowhead_start: p.arrowhead_start,
            arrowhead_end: p.arrowhead_end,
            dash: p.dash,
        }
    }
}

impl CurrentStyle {
    pub fn to_preset(&self, tool: Tool) -> StylePreset {
        StylePreset {
            tool,
            swatch: self.swatch.clone(),
            stroke_width: self.stroke_width,
            redaction_density: self.redaction_density,
            text_font_family: self.font_family.clone(),
            text_font_size: self.font_size,
            text_bold: self.bold,
            text_italic: self.italic,
            text_underline: self.underline,
            text_align: self.align,
            text_fill: self.text_fill.clone(),
            text_border: self.text_border.clone(),
            arrowhead_start: self.arrowhead_start,
            arrowhead_end: self.arrowhead_end,
            dash: self.dash,
            highlighter_straight: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StylePatch {
    Swatch(Swatch),
    /// Slider units.
    StrokeWidth(f64),
    RedactionDensity(f64),
    FontFamily(FontFamily),
    FontSize(f64),
    Bold(bool),
    Italic(bool),
    Underline(bool),
    Align(TextAlign),
    TextFill(Option<Swatch>),
    TextBorder(Option<TextBorder>),
    ArrowheadStart(Arrowhead),
    ArrowheadEnd(Arrowhead),
    Dash(DashStyle),
}

impl StylePatch {
    /// Text patches reach the selection only when it is exactly one text shape.
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            Self::FontFamily(_)
                | Self::FontSize(_)
                | Self::Bold(_)
                | Self::Italic(_)
                | Self::Underline(_)
                | Self::Align(_)
                | Self::TextFill(_)
                | Self::TextBorder(_)
        )
    }

    /// None for a non-finite number, which would poison the document.
    pub(crate) fn normalized(self) -> Option<Self> {
        match self {
            Self::StrokeWidth(w) if !w.is_finite() => None,
            Self::RedactionDensity(d) if !d.is_finite() => None,
            Self::RedactionDensity(d) => Some(Self::RedactionDensity(d.clamp(0.0, 1.0))),
            Self::FontSize(s) if !s.is_finite() => None,
            Self::FontSize(s) => Some(Self::FontSize(s.max(MIN_FONT_SIZE))),
            Self::TextBorder(Some(b)) if !b.width.is_finite() || b.width < 0.0 => None,
            other => Some(other),
        }
    }

    pub(crate) fn apply_to_current(&self, c: &mut CurrentStyle) {
        match self {
            Self::Swatch(s) => c.swatch = s.clone(),
            Self::StrokeWidth(w) => c.stroke_width = *w,
            Self::RedactionDensity(d) => c.redaction_density = *d,
            Self::FontFamily(f) => c.font_family = f.clone(),
            Self::FontSize(s) => c.font_size = *s,
            Self::Bold(b) => c.bold = *b,
            Self::Italic(b) => c.italic = *b,
            Self::Underline(b) => c.underline = *b,
            Self::Align(a) => c.align = *a,
            Self::TextFill(f) => c.text_fill = f.clone(),
            Self::TextBorder(b) => c.text_border = b.clone(),
            Self::ArrowheadStart(h) => c.arrowhead_start = *h,
            Self::ArrowheadEnd(h) => c.arrowhead_end = *h,
            Self::Dash(d) => c.dash = *d,
        }
    }

    pub(crate) fn apply_to_shape(&self, shape: &mut AnnoShape, page_stroke_width: f64) {
        match (self, &mut shape.kind) {
            (Self::Swatch(s), ShapeKind::Geo(p)) => p.swatch = s.clone(),
            (Self::Swatch(s), ShapeKind::Draw(p)) => p.swatch = s.clone(),
            (Self::Swatch(s), ShapeKind::Arrow(p)) => p.swatch = s.clone(),
            (Self::Swatch(s), ShapeKind::Text(p)) => p.swatch = s.clone(),
            (Self::Swatch(s), ShapeKind::Numbered(p)) => p.swatch = s.clone(),
            (Self::Swatch(s), ShapeKind::Highlighter(p)) => p.swatch = s.clone(),
            (Self::StrokeWidth(_), ShapeKind::Geo(p)) => p.stroke_width = page_stroke_width,
            (Self::StrokeWidth(_), ShapeKind::Draw(p)) => p.stroke_width = page_stroke_width,
            (Self::StrokeWidth(_), ShapeKind::Arrow(p)) => p.stroke_width = page_stroke_width,
            (Self::StrokeWidth(_), ShapeKind::Highlighter(p)) => {
                p.stroke_width = page_stroke_width * HIGHLIGHTER_WIDTH_FACTOR
            }
            (Self::RedactionDensity(d), ShapeKind::Redaction(p)) => p.density = *d,
            (Self::FontFamily(f), ShapeKind::Text(p)) => p.font_family = f.clone(),
            (Self::FontSize(s), ShapeKind::Text(p)) => p.font_size = *s,
            (Self::Bold(b), ShapeKind::Text(p)) => p.is_bold = *b,
            (Self::Italic(b), ShapeKind::Text(p)) => p.is_italic = *b,
            (Self::Underline(b), ShapeKind::Text(p)) => p.is_underline = *b,
            (Self::Align(a), ShapeKind::Text(p)) => p.align = *a,
            (Self::TextFill(f), ShapeKind::Text(p)) => p.fill = f.clone(),
            (Self::TextBorder(b), ShapeKind::Text(p)) => p.border = b.clone(),
            // A line is an arrow with no heads; giving it one would turn it into an arrow.
            (Self::ArrowheadStart(h), ShapeKind::Arrow(p)) if has_heads(p) => {
                p.arrowhead_start = *h
            }
            (Self::ArrowheadEnd(h), ShapeKind::Arrow(p)) if has_heads(p) => p.arrowhead_end = *h,
            (Self::Dash(d), ShapeKind::Arrow(p)) => p.dash = *d,
            _ => {}
        }
    }
}

fn has_heads(p: &ArrowProps) -> bool {
    p.arrowhead_start != Arrowhead::None || p.arrowhead_end != Arrowhead::None
}
