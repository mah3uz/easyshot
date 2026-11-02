use std::sync::{Arc, Mutex, MutexGuard};

use es_geom::Size;
use es_model::shape::{self, TextProps};
use es_text::{FontFamily, TextAlign, TextEngine, TextLayout, TextStyle};

use crate::{GlyphOutline, GlyphOutliner, TextMeasure};

pub struct BundledText {
    engine: Mutex<TextEngine>,
}

impl BundledText {
    pub fn new(engine: TextEngine) -> Self {
        Self {
            engine: Mutex::new(engine),
        }
    }

    pub fn bundled() -> Self {
        Self::new(TextEngine::bundled())
    }

    /// The same layout the glyph outlines come from, for caret and selection geometry.
    pub fn layout(&self, props: &TextProps) -> Arc<TextLayout> {
        self.engine()
            .measure(&props.text, &style(props), wrap(props))
    }

    fn engine(&self) -> MutexGuard<'_, TextEngine> {
        // The caches stay valid after a panic elsewhere, so a poisoned lock is still usable.
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn style(props: &TextProps) -> TextStyle {
    TextStyle {
        family: match props.font_family {
            shape::FontFamily::Pro => FontFamily::Pro,
            shape::FontFamily::Compact => FontFamily::Compact,
            shape::FontFamily::Rounded => FontFamily::Rounded,
        },
        size: props.font_size,
        bold: props.is_bold,
        italic: props.is_italic,
        underline: props.is_underline,
        align: match props.align {
            shape::TextAlign::Start => TextAlign::Start,
            shape::TextAlign::Middle => TextAlign::Middle,
            shape::TextAlign::End => TextAlign::End,
        },
    }
}

fn wrap(props: &TextProps) -> Option<f64> {
    (!props.auto_size).then_some(props.w)
}

impl TextMeasure for BundledText {
    fn measure(&self, props: &TextProps) -> Size {
        self.engine()
            .measure(&props.text, &style(props), wrap(props))
            .size
    }
}

impl GlyphOutliner for BundledText {
    fn outline(&self, props: &TextProps) -> GlyphOutline {
        let mut engine = self.engine();
        let layout = engine.measure(&props.text, &style(props), wrap(props));
        let outline = engine.outline(&layout);
        GlyphOutline {
            glyphs: outline.path.clone(),
            underline: outline.underlines.clone(),
        }
    }
}
