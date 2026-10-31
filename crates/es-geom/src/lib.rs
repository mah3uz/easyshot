pub mod arrow;
pub mod bbox;
pub mod freehand;
pub mod geometry2d;
pub mod intersect;
pub mod mat;
pub mod math;
pub mod path;
pub mod vec;

pub use bbox::BBox;
pub use kurbo::{Affine, BezPath, Point, Rect, Size};
pub use mat::Mat;
pub use vec::PVec;

use serde::{Deserialize, Serialize};

/// sRGB, straight (non-premultiplied) alpha, each channel 0.0..=1.0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_roundtrips_through_json() {
        let c = Rgba::new(0.25, 0.5, 0.75, 0.5);
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Rgba>(&s).unwrap(), c);
    }
}
