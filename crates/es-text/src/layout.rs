use std::ops::Range;

use cosmic_text::CacheKey;
use kurbo::{BezPath, Rect, Size};

use crate::TextStyle;

#[derive(Debug, Clone, PartialEq)]
pub struct LineBox {
    pub range: Range<usize>,
    pub top: f64,
    pub height: f64,
    pub baseline: f64,
    pub x: f64,
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedGlyph {
    pub line: usize,
    pub range: Range<usize>,
    pub x: f64,
    pub advance: f64,
    pub(crate) key: CacheKey,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub size: Size,
    pub lines: Vec<LineBox>,
    pub glyphs: Vec<PlacedGlyph>,
    pub style: TextStyle,
    pub text: String,
    pub wrap: Option<f64>,
    pub(crate) underline: Option<(f64, f64)>,
}

impl TextLayout {
    pub fn caret_x(&self, index: usize) -> (usize, f64) {
        let line = self.line_of(index);
        let lb = &self.lines[line];
        let mut x = lb.x;
        for g in self.glyphs.iter().filter(|g| g.line == line) {
            if index <= g.range.start {
                return (line, g.x);
            }
            if index < g.range.end {
                let span = (g.range.end - g.range.start) as f64;
                return (
                    line,
                    g.x + g.advance * (index - g.range.start) as f64 / span,
                );
            }
            x = g.x + g.advance;
        }
        (line, x)
    }

    pub fn caret_rect(&self, index: usize, width: f64) -> Rect {
        let (line, x) = self.caret_x(index);
        let lb = &self.lines[line];
        Rect::new(x, lb.top, x + width, lb.top + lb.height)
    }

    pub fn hit_test(&self, x: f64, y: f64) -> usize {
        let line = self
            .lines
            .iter()
            .position(|l| y < l.top + l.height)
            .unwrap_or(self.lines.len() - 1);
        let mut best = self.lines[line].range.start;
        for g in self.glyphs.iter().filter(|g| g.line == line) {
            if x < g.x + g.advance / 2.0 {
                return g.range.start;
            }
            best = g.range.end;
        }
        best
    }

    /// True when some glyph is .notdef, which draws as a box: no font in the set has that character.
    pub fn has_missing_glyphs(&self) -> bool {
        self.glyphs.iter().any(|g| g.key.glyph_id == 0)
    }

    fn line_of(&self, index: usize) -> usize {
        self.lines
            .iter()
            .position(|l| l.range.start <= index && index < l.range.end)
            .or_else(|| self.lines.iter().position(|l| index <= l.range.end))
            .unwrap_or(self.lines.len() - 1)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub path: BezPath,
    pub underlines: Vec<Rect>,
}
