use crate::shape::{FontFamily, TextAlign, Tool};
use crate::swatch::Swatch;
use serde::{Deserialize, Serialize};

/// The editor's last-used drawing style, restored on every editor open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StylePreset {
    pub tool: Tool,
    pub swatch: Swatch,
    /// Slider units, not page pixels; see `page_stroke_width`.
    pub stroke_width: f64,
    pub redaction_density: f64,
    pub text_font_family: FontFamily,
    pub text_font_size: f64,
    pub text_bold: bool,
    pub text_italic: bool,
    pub text_underline: bool,
    pub text_align: TextAlign,
}

impl Default for StylePreset {
    fn default() -> Self {
        Self {
            tool: Tool::Rectangle,
            swatch: Swatch::red(),
            stroke_width: 4.0,
            redaction_density: 0.55,
            text_font_family: FontFamily::Pro,
            text_font_size: 48.0,
            text_bold: true,
            text_italic: false,
            text_underline: false,
            text_align: TextAlign::Start,
        }
    }
}

pub const STROKE_SLIDER_MIN: f64 = 1.0;
pub const STROKE_SLIDER_MAX: f64 = 24.0;

/// Slider units to page pixels: the stroke scales with the image's longest edge.
pub fn page_stroke_width(slider: f64, image_w: f64, image_h: f64) -> f64 {
    (slider * image_w.max(image_h) / 900.0).max(1.0)
}

pub fn slider_stroke_width(page: f64, image_w: f64, image_h: f64) -> f64 {
    page * 900.0 / image_w.max(image_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_equal_the_documented_style() {
        let s = StylePreset::default();
        assert_eq!(
            (s.tool, s.stroke_width, s.redaction_density),
            (Tool::Rectangle, 4.0, 0.55)
        );
        assert_eq!(s.swatch, Swatch::red());
        assert_eq!(
            (s.text_font_family, s.text_font_size),
            (FontFamily::Pro, 48.0)
        );
        assert!(s.text_bold && !s.text_italic && !s.text_underline);
        assert_eq!(s.text_align, TextAlign::Start);
    }

    #[test]
    fn default_slider_is_eight_page_pixels_on_an_1800_wide_image() {
        assert_eq!(page_stroke_width(4.0, 1800.0, 1000.0), 8.0);
        assert_eq!(slider_stroke_width(8.0, 1800.0, 1000.0), 4.0);
    }

    #[test]
    fn page_stroke_never_drops_below_one_pixel() {
        assert_eq!(page_stroke_width(1.0, 300.0, 200.0), 1.0);
    }

    #[test]
    fn custom_swatch_and_select_tool_survive_a_round_trip() {
        let s = StylePreset {
            swatch: Swatch::custom(10, 200, 30, 128),
            tool: Tool::Select,
            ..StylePreset::default()
        };
        let back: StylePreset = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!((back.tool, &back.swatch.id), (Tool::Select, &s.swatch.id));
        assert!((back.swatch.alpha - s.swatch.alpha).abs() < 1e-12);
        assert_eq!(back.swatch.to_rgba(), s.swatch.to_rgba());
    }
}
