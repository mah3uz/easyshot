use super::builder::PathBuilder;
use crate::vec::PVec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeoShape {
    Rectangle,
    Ellipse,
}

fn v(x: f64, y: f64) -> PVec {
    PVec::new(x, y)
}

pub fn geo_path(geo: GeoShape, width: f64, height: f64, corner_radius: f64) -> PathBuilder {
    let w = width.max(1.0);
    let h = height.max(1.0);
    match geo {
        GeoShape::Rectangle => {
            let radius = corner_radius.min(w.min(h) / 2.0);
            if radius.is_nan() || radius <= 0.5 {
                return box_path(w, h);
            }
            let mut p = PathBuilder::new();
            p.move_to(v(radius, 0.0))
                .line_to(v(w - radius, 0.0))
                .circular_arc(radius, false, true, v(w, radius))
                .line_to(v(w, h - radius))
                .circular_arc(radius, false, true, v(w - radius, h))
                .line_to(v(radius, h))
                .circular_arc(radius, false, true, v(0.0, h - radius))
                .line_to(v(0.0, radius))
                .circular_arc(radius, false, true, v(radius, 0.0))
                .close();
            p
        }
        GeoShape::Ellipse => {
            let (cx, cy) = (w / 2.0, h / 2.0);
            let mut p = PathBuilder::new();
            p.move_to(v(0.0, cy))
                .arc(cx, cy, false, true, 0.0, v(w, cy))
                .arc(cx, cy, false, true, 0.0, v(0.0, cy))
                .close();
            p
        }
    }
}

pub fn box_path(width: f64, height: f64) -> PathBuilder {
    let mut p = PathBuilder::new();
    p.move_to(v(0.0, 0.0))
        .line_to(v(width, 0.0))
        .line_to(v(width, height))
        .line_to(v(0.0, height))
        .close();
    p
}
