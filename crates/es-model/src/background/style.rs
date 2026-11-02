use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundColor {
    pub id: String,
    pub title: String,
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Default for BackgroundColor {
    fn default() -> Self {
        Self::new("black", "Black", 0.02, 0.02, 0.024)
    }
}

impl BackgroundColor {
    pub fn new(id: &str, title: &str, red: f64, green: f64, blue: f64) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            red,
            green,
            blue,
            alpha: 1.0,
        }
    }

    pub fn custom(red: f64, green: f64, blue: f64) -> Self {
        Self::new("custom", "Custom", red, green, blue)
    }

    pub fn plain_presets() -> Vec<BackgroundColor> {
        const TABLE: [(&str, &str, f64, f64, f64); 16] = [
            ("black", "Black", 0.02, 0.02, 0.024),
            ("white", "White", 0.96, 0.96, 0.94),
            ("graphite", "Graphite", 0.17, 0.18, 0.21),
            ("red", "Red", 0.94, 0.23, 0.28),
            ("orange", "Orange", 0.97, 0.52, 0.16),
            ("yellow", "Yellow", 0.96, 0.73, 0.23),
            ("green", "Green", 0.23, 0.61, 0.36),
            ("blue", "Blue", 0.16, 0.50, 0.88),
            ("purple", "Purple", 0.48, 0.26, 0.91),
            ("blush", "Blush", 0.93, 0.66, 0.62),
            ("mint", "Mint", 0.66, 0.90, 0.73),
            ("sky", "Sky", 0.63, 0.79, 0.94),
            ("lavender", "Lavender", 0.80, 0.76, 0.92),
            ("peach", "Peach", 0.98, 0.80, 0.69),
            ("sage", "Sage", 0.74, 0.82, 0.70),
            ("sand", "Sand", 0.91, 0.87, 0.76),
        ];
        TABLE
            .iter()
            .map(|&(id, title, r, g, b)| Self::new(id, title, r, g, b))
            .collect()
    }
}

/// Gradient endpoints in the unit square, top left origin.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundGradient {
    pub id: String,
    pub title: String,
    pub colors: Vec<BackgroundColor>,
    pub start: UnitPoint,
    pub end: UnitPoint,
}

impl Default for BackgroundGradient {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            colors: Vec::new(),
            start: UnitPoint { x: 0.0, y: 0.0 },
            end: UnitPoint { x: 1.0, y: 1.0 },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackgroundStyle {
    #[default]
    None,
    Solid(BackgroundColor),
    Gradient(BackgroundGradient),
    Wallpaper {
        path: PathBuf,
    },
}
