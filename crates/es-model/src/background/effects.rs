use crate::swatch::Swatch;
use es_geom::Rgba;
use serde::{Deserialize, Serialize};

const EPSILON: f64 = 0.0001;

fn near_zero(v: f64) -> bool {
    v.abs() <= EPSILON
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BorderSettings {
    pub is_enabled: bool,
    pub color: Swatch,
    /// A fraction of the screenshot's shortest edge, so a preset keeps its weight across sizes.
    pub thickness: f64,
    pub opacity: f64,
}

impl Default for BorderSettings {
    fn default() -> Self {
        Self {
            is_enabled: false,
            color: Swatch::white(),
            thickness: 0.012,
            opacity: 1.0,
        }
    }
}

impl BorderSettings {
    pub fn is_visible(&self) -> bool {
        self.is_enabled && self.thickness > EPSILON && self.opacity > EPSILON
    }

    pub fn pixel_thickness(&self, width: f64, height: f64) -> f64 {
        if !self.is_visible() || width <= 0.0 || height <= 0.0 {
            return 0.0;
        }
        self.thickness.max(0.0) * width.min(height)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraSettings {
    /// A fraction of the final canvas dimensions.
    pub pan_x: f64,
    pub pan_y: f64,
    pub tilt_x_degrees: f64,
    pub tilt_y_degrees: f64,
    pub rotation_x_degrees: f64,
    pub rotation_y_degrees: f64,
    pub roll_degrees: f64,
    /// Lens angle; controls the strength of perspective.
    pub field_of_view_degrees: f64,
    /// Final scale around the card centre. Rotation never changes it.
    pub zoom: f64,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            tilt_x_degrees: 0.0,
            tilt_y_degrees: 0.0,
            rotation_x_degrees: 0.0,
            rotation_y_degrees: 0.0,
            roll_degrees: 0.0,
            field_of_view_degrees: 24.0,
            zoom: 1.0,
        }
    }
}

impl CameraSettings {
    pub fn is_default(&self) -> bool {
        !self.has_effect() && (self.field_of_view_degrees - 24.0).abs() <= EPSILON
    }

    /// The field of view alone does not move the card, so it is not an effect.
    pub fn has_effect(&self) -> bool {
        !near_zero(self.pan_x)
            || !near_zero(self.pan_y)
            || !near_zero(self.tilt_x_degrees)
            || !near_zero(self.tilt_y_degrees)
            || !near_zero(self.rotation_x_degrees)
            || !near_zero(self.rotation_y_degrees)
            || !near_zero(self.roll_degrees)
            || (self.zoom - 1.0).abs() > EPSILON
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlurMode {
    #[default]
    Radial,
    Directional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlurEdgeMode {
    Clipped,
    #[default]
    Bleed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgressiveBlurSettings {
    pub is_enabled: bool,
    pub edge_mode: BlurEdgeMode,
    pub mode: BlurMode,
    pub strength: f64,
    /// Width of the sharp-to-blurred transition, 0..=1.
    pub falloff: f64,
    /// Size of the sharp focal area, 0..=1 up to the farthest image edge.
    pub focus_size: f64,
    /// Focal point in 0..=1, top left origin.
    pub focus_x: f64,
    pub focus_y: f64,
    /// Direction of the in-focus band; 0 is horizontal.
    pub direction_degrees: f64,
}

impl Default for ProgressiveBlurSettings {
    fn default() -> Self {
        Self {
            is_enabled: false,
            edge_mode: BlurEdgeMode::Bleed,
            mode: BlurMode::Radial,
            strength: 18.0,
            falloff: 0.55,
            focus_size: 0.45,
            focus_x: 0.5,
            focus_y: 0.5,
            direction_degrees: 0.0,
        }
    }
}

impl ProgressiveBlurSettings {
    pub fn is_active(&self) -> bool {
        self.is_enabled && self.strength > 0.01
    }
}

fn mercury() -> Rgba {
    Rgba::new(0.90, 0.90, 0.90, 1.0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WatermarkSettings {
    pub is_enabled: bool,
    pub text: String,
    pub density: f64,
    pub font_size: f64,
    pub rotation_degrees: f64,
    pub opacity: f64,
    #[serde(default = "mercury")]
    pub color: Rgba,
}

impl Default for WatermarkSettings {
    fn default() -> Self {
        Self {
            is_enabled: false,
            text: String::new(),
            density: 4.0,
            font_size: 72.0,
            rotation_degrees: 45.0,
            opacity: 0.18,
            color: mercury(),
        }
    }
}

impl WatermarkSettings {
    pub fn is_visible(&self) -> bool {
        !self.text.trim().is_empty() && self.opacity > 0.0
    }
}
