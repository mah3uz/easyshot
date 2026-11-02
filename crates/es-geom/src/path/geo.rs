use super::builder::PathBuilder;
use crate::vec::PVec;
use kurbo::Rect;
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

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PerCornerRadii {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_left: f64,
    pub bottom_right: f64,
}

impl PerCornerRadii {
    pub fn uniform(r: f64) -> Self {
        Self {
            top_left: r,
            top_right: r,
            bottom_left: r,
            bottom_right: r,
        }
    }

    pub fn is_uniform(&self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_left
            && self.bottom_left == self.bottom_right
    }

    pub fn outset_by(&self, amount: f64) -> Self {
        let grow = |r: f64| if r > 0.0 { r + amount } else { 0.0 };
        Self {
            top_left: grow(self.top_left),
            top_right: grow(self.top_right),
            bottom_left: grow(self.bottom_left),
            bottom_right: grow(self.bottom_right),
        }
    }

    pub fn path_in(&self, rect: Rect) -> PathBuilder {
        let (x0, y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
        let limit = rect.width().min(rect.height()) / 2.0;
        let tl = self.top_left.min(limit);
        let tr = self.top_right.min(limit);
        let bl = self.bottom_left.min(limit);
        let br = self.bottom_right.min(limit);

        let mut p = PathBuilder::new();
        p.move_to(v(x0 + tl, y0)).line_to(v(x1 - tr, y0));
        if tr > 0.0 {
            p.circular_arc(tr, false, true, v(x1, y0 + tr));
        }
        p.line_to(v(x1, y1 - br));
        if br > 0.0 {
            p.circular_arc(br, false, true, v(x1 - br, y1));
        }
        p.line_to(v(x0 + bl, y1));
        if bl > 0.0 {
            p.circular_arc(bl, false, true, v(x0, y1 - bl));
        }
        p.line_to(v(x0, y0 + tl));
        if tl > 0.0 {
            p.circular_arc(tl, false, true, v(x0 + tl, y0));
        }
        p.close();
        p
    }
}
