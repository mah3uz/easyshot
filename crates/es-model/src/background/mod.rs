mod effects;
mod gradients;
mod presets;
mod style;

pub use effects::{
    BlurEdgeMode, BlurMode, BorderSettings, CameraSettings, ProgressiveBlurSettings,
    WatermarkSettings,
};
pub use gradients::{
    GradientLibrary, MAX_GRADIENT_NAME_CHARS, MAX_GRADIENT_STOPS, MIN_GRADIENT_STOPS,
    clean_gradient_name,
};
pub use presets::{BackgroundPreset, MAX_PRESET_NAME_CHARS, PresetLibrary};
pub use style::{BackgroundColor, BackgroundGradient, BackgroundStyle, UnitPoint};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowStyle {
    #[default]
    Soft,
    Long,
    Glow,
    Crisp,
}

impl ShadowStyle {
    pub fn radius_scale(self) -> f64 {
        match self {
            ShadowStyle::Soft => 1.0,
            ShadowStyle::Long => 1.2,
            ShadowStyle::Glow => 1.6,
            ShadowStyle::Crisp => 0.8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatio {
    #[default]
    Auto,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
}

impl AspectRatio {
    pub fn value(self) -> Option<f64> {
        match self {
            AspectRatio::Auto => None,
            AspectRatio::Square => Some(1.0),
            AspectRatio::FourThree => Some(4.0 / 3.0),
            AspectRatio::ThreeTwo => Some(3.0 / 2.0),
            AspectRatio::SixteenNine => Some(16.0 / 9.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    TopLeading,
    Top,
    TopTrailing,
    Leading,
    #[default]
    Center,
    Trailing,
    BottomLeading,
    Bottom,
    BottomTrailing,
}

/// Multipliers (0 or 1) for the card's corner radii; a corner on a stuck edge is square.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CornerMultipliers {
    pub top_left: u8,
    pub top_right: u8,
    pub bottom_left: u8,
    pub bottom_right: u8,
}

impl Alignment {
    pub fn x_factor(self) -> f64 {
        match self {
            Alignment::TopLeading | Alignment::Leading | Alignment::BottomLeading => 0.0,
            Alignment::Top | Alignment::Center | Alignment::Bottom => 0.5,
            Alignment::TopTrailing | Alignment::Trailing | Alignment::BottomTrailing => 1.0,
        }
    }

    pub fn y_factor(self) -> f64 {
        match self {
            Alignment::TopLeading | Alignment::Top | Alignment::TopTrailing => 0.0,
            Alignment::Leading | Alignment::Center | Alignment::Trailing => 0.5,
            Alignment::BottomLeading | Alignment::Bottom | Alignment::BottomTrailing => 1.0,
        }
    }

    pub fn sticks_to_top(self) -> bool {
        self.y_factor() == 0.0
    }

    pub fn sticks_to_bottom(self) -> bool {
        self.y_factor() == 1.0
    }

    pub fn sticks_to_leading(self) -> bool {
        self.x_factor() == 0.0
    }

    pub fn sticks_to_trailing(self) -> bool {
        self.x_factor() == 1.0
    }

    pub fn corner_multipliers(self) -> CornerMultipliers {
        let m = |top_left, top_right, bottom_left, bottom_right| CornerMultipliers {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        };
        match self {
            Alignment::Center => m(1, 1, 1, 1),
            Alignment::Top => m(0, 0, 1, 1),
            Alignment::Bottom => m(1, 1, 0, 0),
            Alignment::Leading => m(0, 1, 0, 1),
            Alignment::Trailing => m(1, 0, 1, 0),
            Alignment::TopLeading => m(0, 0, 0, 1),
            Alignment::TopTrailing => m(0, 0, 1, 0),
            Alignment::BottomLeading => m(0, 1, 0, 0),
            Alignment::BottomTrailing => m(1, 0, 0, 0),
        }
    }
}

/// Padding, corner radius and shadow are fractions of the image's longest edge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundSettings {
    pub style: BackgroundStyle,
    pub padding: f64,
    pub corner_radius: f64,
    pub shadow: f64,
    pub shadow_style: ShadowStyle,
    pub border: BorderSettings,
    pub aspect_ratio: AspectRatio,
    pub alignment: Alignment,
    pub camera: CameraSettings,
    pub progressive_blur: ProgressiveBlurSettings,
    pub watermark: WatermarkSettings,
}

impl Default for BackgroundSettings {
    fn default() -> Self {
        Self {
            style: BackgroundStyle::None,
            padding: 0.08,
            corner_radius: 0.018,
            shadow: 0.36,
            shadow_style: ShadowStyle::Soft,
            border: BorderSettings::default(),
            aspect_ratio: AspectRatio::Auto,
            alignment: Alignment::Center,
            camera: CameraSettings::default(),
            progressive_blur: ProgressiveBlurSettings::default(),
            watermark: WatermarkSettings::default(),
        }
    }
}

impl BackgroundSettings {
    pub fn is_enabled(&self) -> bool {
        self.style != BackgroundStyle::None
    }

    /// Camera transforms and scene blur need a stage even without a background, so
    /// their pixels have room instead of being cropped to the screenshot.
    pub fn uses_canvas_layout(&self) -> bool {
        self.is_enabled()
            || self.camera.has_effect()
            || (self.progressive_blur.is_active()
                && self.progressive_blur.edge_mode == BlurEdgeMode::Bleed)
    }

    /// An outer border needs a canvas large enough to keep the ring.
    pub fn requires_canvas_layout(&self) -> bool {
        self.uses_canvas_layout() || self.border.is_visible()
    }

    /// Once the card leaves the flat plane, stuck edges no longer describe its projection.
    pub fn effective_canvas_alignment(&self) -> Alignment {
        if self.camera.has_effect() || !self.uses_canvas_layout() {
            Alignment::Center
        } else {
            self.alignment
        }
    }

    pub fn has_renderable_content(&self) -> bool {
        self.is_enabled()
            || self.camera.has_effect()
            || self.progressive_blur.is_active()
            || self.border.is_visible()
            || self.watermark.is_visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_equal_the_documented_look() {
        let b = BackgroundSettings::default();
        assert_eq!(b.style, BackgroundStyle::None);
        assert_eq!((b.padding, b.corner_radius, b.shadow), (0.08, 0.018, 0.36));
        assert_eq!(b.shadow_style, ShadowStyle::Soft);
        assert_eq!(
            (b.aspect_ratio, b.alignment),
            (AspectRatio::Auto, Alignment::Center)
        );
        assert!(!b.border.is_enabled && b.border.thickness == 0.012 && b.border.opacity == 1.0);
        assert_eq!(b.border.color, crate::Swatch::white());
        assert!(
            b.camera.is_default() && b.camera.field_of_view_degrees == 24.0 && b.camera.zoom == 1.0
        );
        let p = &b.progressive_blur;
        assert_eq!(
            (p.edge_mode, p.mode),
            (BlurEdgeMode::Bleed, BlurMode::Radial)
        );
        assert_eq!((p.strength, p.falloff, p.focus_size), (18.0, 0.55, 0.45));
        assert_eq!((p.focus_x, p.focus_y, p.direction_degrees), (0.5, 0.5, 0.0));
        let w = &b.watermark;
        assert_eq!(
            (w.density, w.font_size, w.rotation_degrees, w.opacity),
            (4.0, 72.0, 45.0, 0.18)
        );
        assert_eq!(w.color, es_geom::Rgba::new(0.9, 0.9, 0.9, 1.0));
        assert!(!b.has_renderable_content());
    }

    #[test]
    fn a_stage_is_needed_for_camera_and_scene_blur_but_not_for_a_clipped_blur() {
        let mut b = BackgroundSettings::default();
        assert!(!b.uses_canvas_layout());
        b.camera.zoom = 1.2;
        assert!(b.uses_canvas_layout());
        assert_eq!(b.effective_canvas_alignment(), Alignment::Center);

        let mut b = BackgroundSettings::default();
        b.progressive_blur.is_enabled = true;
        assert!(b.uses_canvas_layout());
        b.progressive_blur.edge_mode = BlurEdgeMode::Clipped;
        assert!(!b.uses_canvas_layout());
        assert!(b.has_renderable_content());
    }

    #[test]
    fn border_alone_requires_a_canvas_but_does_not_use_the_stage() {
        let mut b = BackgroundSettings::default();
        b.border.is_enabled = true;
        assert!(!b.uses_canvas_layout());
        assert!(b.requires_canvas_layout());
        assert_eq!(b.border.pixel_thickness(1000.0, 500.0), 0.012 * 500.0);
        b.border.opacity = 0.0;
        assert_eq!(b.border.pixel_thickness(1000.0, 500.0), 0.0);
    }

    #[test]
    fn alignment_only_applies_on_the_flat_stage() {
        let mut b = BackgroundSettings {
            alignment: Alignment::BottomLeading,
            ..BackgroundSettings::default()
        };
        assert_eq!(b.effective_canvas_alignment(), Alignment::Center);
        b.style = BackgroundStyle::Solid(BackgroundColor::custom(0.1, 0.2, 0.3));
        assert_eq!(b.effective_canvas_alignment(), Alignment::BottomLeading);
    }

    #[test]
    fn stuck_edges_square_the_corners_they_touch() {
        let m = Alignment::TopLeading.corner_multipliers();
        assert_eq!(
            (m.top_left, m.top_right, m.bottom_left, m.bottom_right),
            (0, 0, 0, 1)
        );
        assert!(Alignment::TopLeading.sticks_to_top() && Alignment::TopLeading.sticks_to_leading());
        assert!(Alignment::Bottom.sticks_to_bottom() && !Alignment::Bottom.sticks_to_leading());
        assert_eq!(
            (
                Alignment::Trailing.x_factor(),
                Alignment::Trailing.y_factor()
            ),
            (1.0, 0.5)
        );
        assert_eq!(Alignment::Center.corner_multipliers().top_left, 1);
    }

    #[test]
    fn watermark_needs_text_and_opacity_to_be_visible() {
        let mut w = WatermarkSettings::default();
        assert!(!w.is_visible());
        w.text = "  ".into();
        assert!(!w.is_visible());
        w.text = "draft".into();
        assert!(w.is_visible());
        w.opacity = 0.0;
        assert!(!w.is_visible());
    }

    #[test]
    fn aspect_ratio_values() {
        assert_eq!(AspectRatio::Auto.value(), None);
        assert_eq!(AspectRatio::SixteenNine.value(), Some(16.0 / 9.0));
        assert_eq!(ShadowStyle::Glow.radius_scale(), 1.6);
    }

    #[test]
    fn partial_json_keeps_the_other_defaults_and_ignores_unknown_keys() {
        let b: BackgroundSettings =
            serde_json::from_str(r#"{"padding":0.2,"unknown":1,"camera":{"zoom":2.0}}"#).unwrap();
        assert_eq!(b.padding, 0.2);
        assert_eq!(b.corner_radius, 0.018);
        assert_eq!(b.camera.zoom, 2.0);
        assert_eq!(b.camera.field_of_view_degrees, 24.0);
    }

    #[test]
    fn style_variants_round_trip_with_a_type_tag() {
        let styles = [
            BackgroundStyle::None,
            BackgroundStyle::Solid(BackgroundColor::custom(0.2, 0.4, 0.6)),
            BackgroundStyle::Gradient(BackgroundGradient {
                id: "g".into(),
                title: "G".into(),
                colors: BackgroundColor::plain_presets()
                    .into_iter()
                    .take(3)
                    .collect(),
                start: UnitPoint { x: 0.0, y: 0.0 },
                end: UnitPoint { x: 1.0, y: 0.5 },
            }),
            BackgroundStyle::Wallpaper {
                path: "/tmp/wall.jpg".into(),
            },
        ];
        let tags = ["none", "solid", "gradient", "wallpaper"];
        for (s, tag) in styles.iter().zip(tags) {
            let v = serde_json::to_value(s).unwrap();
            assert_eq!(v["type"], tag);
            assert_eq!(&serde_json::from_value::<BackgroundStyle>(v).unwrap(), s);
        }
    }
}
