use crate::bbox::BBox;
use crate::intersect;
use crate::mat::Mat;
use crate::math::point_in_polygon;
use crate::vec::PVec;
use std::sync::Arc;

mod shapes;
#[cfg(test)]
mod tests;

pub use shapes::{Arc2d, Circle2d, Edge2d, Ellipse2d, Polyline2d};

pub trait Geometry2d: Send + Sync {
    fn is_filled(&self) -> bool;
    fn is_closed(&self) -> bool;
    fn vertices(&self) -> &[PVec];

    fn bounds(&self) -> BBox {
        BBox::from_points(self.vertices())
    }

    fn center(&self) -> PVec {
        self.bounds().center()
    }

    fn length(&self) -> f64 {
        let verts = self.vertices();
        if verts.len() < 2 {
            return 0.0;
        }
        let mut total: f64 = verts.windows(2).map(|w| PVec::dist(w[0], w[1])).sum();
        if self.is_closed() {
            total += PVec::dist(verts[verts.len() - 1], verts[0]);
        }
        total
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        let verts = self.vertices();
        let Some(&first) = verts.first() else {
            return point;
        };
        let limit = if self.is_closed() {
            verts.len()
        } else {
            verts.len() - 1
        };
        let mut best = first;
        let mut best_dist = PVec::dist2(point, best);
        for i in 0..limit {
            let p = PVec::nearest_point_on_line_segment(
                verts[i],
                verts[(i + 1) % verts.len()],
                point,
                true,
            );
            let d = PVec::dist2(point, p);
            if d < best_dist {
                best = p;
                best_dist = d;
            }
        }
        best
    }

    fn hit_test_point(&self, point: PVec, margin: f64, hit_inside: bool) -> bool {
        if self.is_closed()
            && (self.is_filled() || hit_inside)
            && point_in_polygon(point, self.vertices())
        {
            return true;
        }
        PVec::dist2(point, self.nearest_point(point)) <= margin * margin
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        let d = PVec::dist(point, self.nearest_point(point));
        if self.is_closed()
            && (self.is_filled() || hit_inside)
            && point_in_polygon(point, self.vertices())
        {
            return -d;
        }
        d
    }

    fn intersect_line_segment(&self, a: PVec, b: PVec) -> Vec<PVec> {
        if self.is_closed() {
            intersect::line_segment_polygon(a, b, self.vertices())
        } else {
            intersect::line_segment_polyline(a, b, self.vertices())
        }
    }

    fn intersect_circle(&self, center: PVec, radius: f64) -> Vec<PVec> {
        if self.is_closed() {
            intersect::circle_polygon(center, radius, self.vertices())
        } else {
            intersect::circle_polyline(center, radius, self.vertices())
        }
    }

    fn overlaps_polygon(&self, polygon: &[PVec]) -> bool {
        let verts = self.vertices();
        if verts.iter().any(|v| point_in_polygon(*v, polygon)) {
            return true;
        }
        if self.is_closed() {
            if self.is_filled() {
                if point_in_polygon(self.center(), polygon) {
                    return true;
                }
                if polygon.iter().all(|p| point_in_polygon(*p, verts)) {
                    return true;
                }
            }
            intersect::polygons_intersect(polygon, verts)
        } else {
            intersect::polygon_intersects_polyline(polygon, verts)
        }
    }
}

pub struct Group2d {
    children: Vec<Box<dyn Geometry2d>>,
    vertices: Vec<PVec>,
}

impl Group2d {
    pub fn new(children: Vec<Box<dyn Geometry2d>>) -> Self {
        let vertices = children
            .iter()
            .flat_map(|c| c.vertices().iter().copied())
            .collect();
        Self { children, vertices }
    }

    pub fn children(&self) -> &[Box<dyn Geometry2d>] {
        &self.children
    }
}

impl Geometry2d for Group2d {
    fn is_filled(&self) -> bool {
        false
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn bounds(&self) -> BBox {
        let boxes: Vec<BBox> = self.children.iter().map(|c| c.bounds()).collect();
        BBox::common(&boxes)
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        let mut nearest = point;
        let mut dist = f64::INFINITY;
        for child in &self.children {
            let p = child.nearest_point(point);
            let d = PVec::dist2(p, point);
            if d < dist {
                nearest = p;
                dist = d;
            }
        }
        nearest
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        self.children
            .iter()
            .map(|c| c.distance_to_point(point, hit_inside))
            .fold(f64::INFINITY, f64::min)
    }

    fn hit_test_point(&self, point: PVec, margin: f64, hit_inside: bool) -> bool {
        self.children
            .iter()
            .any(|c| c.hit_test_point(point, margin, hit_inside))
    }

    fn intersect_line_segment(&self, a: PVec, b: PVec) -> Vec<PVec> {
        self.children
            .iter()
            .flat_map(|c| c.intersect_line_segment(a, b))
            .collect()
    }

    fn intersect_circle(&self, center: PVec, radius: f64) -> Vec<PVec> {
        self.children
            .iter()
            .flat_map(|c| c.intersect_circle(center, radius))
            .collect()
    }

    fn overlaps_polygon(&self, polygon: &[PVec]) -> bool {
        self.children.iter().any(|c| c.overlaps_polygon(polygon))
    }
}

pub struct Transformed {
    geometry: Arc<dyn Geometry2d>,
    matrix: Mat,
    inverse: Mat,
    scale: f64,
    vertices: Vec<PVec>,
}

impl Transformed {
    pub fn new(geometry: Arc<dyn Geometry2d>, matrix: Mat) -> Self {
        let vertices = matrix.apply_to_points(geometry.vertices());
        Self {
            inverse: matrix.inverse(),
            scale: matrix.decomposed().scale_x,
            vertices,
            geometry,
            matrix,
        }
    }
}

impl Geometry2d for Transformed {
    fn is_filled(&self) -> bool {
        self.geometry.is_filled()
    }

    fn is_closed(&self) -> bool {
        self.geometry.is_closed()
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        self.matrix.apply_to_point(
            self.geometry
                .nearest_point(self.inverse.apply_to_point(point)),
        )
    }

    fn hit_test_point(&self, point: PVec, margin: f64, hit_inside: bool) -> bool {
        self.geometry.hit_test_point(
            self.inverse.apply_to_point(point),
            margin / self.scale,
            hit_inside,
        )
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        self.geometry
            .distance_to_point(self.inverse.apply_to_point(point), hit_inside)
            * self.scale
    }

    fn intersect_line_segment(&self, a: PVec, b: PVec) -> Vec<PVec> {
        self.matrix
            .apply_to_points(&self.geometry.intersect_line_segment(
                self.inverse.apply_to_point(a),
                self.inverse.apply_to_point(b),
            ))
    }

    fn intersect_circle(&self, center: PVec, radius: f64) -> Vec<PVec> {
        self.matrix.apply_to_points(
            &self
                .geometry
                .intersect_circle(self.inverse.apply_to_point(center), radius / self.scale),
        )
    }
}
