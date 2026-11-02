use super::types::{ArrowBody, ArrowInfo, Arrowhead, Terminal};
use crate::geometry2d::{Arc2d, Circle2d, Edge2d, Geometry2d, Group2d, Polyline2d};
use crate::intersect::circle_circle;
use crate::math::{HALF_PI, clamp};
use crate::path::{CommandOpts, DashStyle, DashedSegment, PathBuilder};
use crate::vec::PVec;
use kurbo::BezPath;
use std::f64::consts::PI;

#[derive(Debug, Clone, PartialEq)]
pub struct HeadPath {
    pub path: BezPath,
    pub filled: bool,
}

/// The tip and a point one head-length back along the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadPoints {
    pub point: PVec,
    pub int: PVec,
}

const NO_JITTER: CommandOpts = CommandOpts {
    offset: Some(0.0),
    roundness: Some(0.0),
};

impl ArrowInfo {
    pub fn head_points(&self, side: Terminal) -> HeadPoints {
        let sw = self.stroke_width;
        let (point, opposite) = match side {
            Terminal::End => (self.end.point, self.start.point),
            Terminal::Start => (self.start.point, self.end.point),
        };
        let mut int = match &self.body {
            ArrowBody::Straight { .. } => {
                let length = clamp(PVec::dist(opposite, point) / 5.0, sw, sw * 3.0);
                PVec::nudge(point, opposite, length)
            }
            ArrowBody::Arc(body) => {
                let length = clamp(body.length.abs() / 5.0, sw, sw * 3.0);
                let Some(handle) = self.handle_arc else {
                    return HeadPoints { point, int: point };
                };
                let crossings = circle_circle(point, length, handle.center, handle.radius);
                let first = match side {
                    Terminal::End => handle.sweep,
                    Terminal::Start => !handle.sweep,
                };
                crossings[if first { 0 } else { 1 }]
            }
        };
        if int.is_nan() {
            int = point;
        }
        HeadPoints { point, int }
    }

    pub fn head_path(&self, side: Terminal) -> Option<HeadPath> {
        let head = match side {
            Terminal::End => self.end.head,
            Terminal::Start => self.start.head,
        };
        if head == Arrowhead::None {
            return None;
        }
        let p = self.head_points(side);
        let (builder, filled) = match head {
            Arrowhead::None => return None,
            Arrowhead::Arrow => (open_arrow(p), false),
            Arrowhead::Triangle => (triangle(p), true),
            Arrowhead::Inverted => (inverted(p), true),
            Arrowhead::Dot => (dot(p), true),
            Arrowhead::Diamond => (diamond(p), true),
            Arrowhead::Square => (square(p), true),
            Arrowhead::Bar => (bar(p), false),
        };
        Some(HeadPath {
            path: builder.to_bez_path(),
            filled,
        })
    }

    pub fn body_builder(&self) -> PathBuilder {
        let mut path = PathBuilder::new();
        path.move_to_with(self.start.point, NO_JITTER);
        match &self.body {
            ArrowBody::Straight { end, .. } => {
                path.line_to_with(*end, NO_JITTER);
            }
            ArrowBody::Arc(arc) => {
                path.arc_with(
                    arc.radius,
                    arc.radius,
                    arc.large_arc,
                    arc.sweep,
                    0.0,
                    self.end.point,
                    NO_JITTER,
                );
            }
        }
        path
    }

    pub fn body_path(&self) -> BezPath {
        self.body_builder().to_bez_path()
    }

    pub fn body_dashes(&self, style: DashStyle) -> Vec<DashedSegment> {
        self.body_builder()
            .dashed_segments(self.stroke_width, style)
    }

    /// The path through the handles, drawn as a guide while the arrow is selected.
    pub fn handles_path(&self) -> BezPath {
        let mut path = PathBuilder::new();
        path.move_to(self.start.handle);
        match self.handle_arc {
            Some(arc) if !self.is_straight() => {
                path.circular_arc(arc.radius, arc.large_arc, arc.sweep, self.end.handle);
            }
            _ => {
                path.line_to(self.end.handle);
            }
        }
        path.to_bez_path()
    }

    /// Hit-test geometry: the body plus both heads.
    pub fn geometry(&self) -> Group2d {
        let mut parts: Vec<Box<dyn Geometry2d>> = vec![self.body_geometry()];
        for side in [Terminal::Start, Terminal::End] {
            if let Some(g) = self.head_geometry(side) {
                parts.push(g);
            }
        }
        Group2d::new(parts)
    }

    fn body_geometry(&self) -> Box<dyn Geometry2d> {
        match (&self.body, self.is_valid) {
            (ArrowBody::Arc(arc), true) => Box::new(Arc2d::new(
                arc.center,
                self.start.point,
                self.end.point,
                arc.sweep,
                arc.large_arc,
            )),
            _ => Box::new(Edge2d::new(self.start.point, self.end.point)),
        }
    }

    fn head_geometry(&self, side: Terminal) -> Option<Box<dyn Geometry2d>> {
        let head = match side {
            Terminal::End => self.end.head,
            Terminal::Start => self.start.head,
        };
        let p = self.head_points(side);
        let (pts, closed) = match head {
            Arrowhead::None => return None,
            Arrowhead::Dot => {
                let a = PVec::lrp(p.point, p.int, 0.45);
                let r = PVec::dist(a, p.point);
                return Some(Box::new(Circle2d::new(a.x - r, a.y - r, r, true)));
            }
            Arrowhead::Arrow => (open_arrow_points(p).to_vec(), false),
            Arrowhead::Triangle => (triangle_points(p).to_vec(), true),
            Arrowhead::Inverted => (inverted_points(p).to_vec(), true),
            Arrowhead::Diamond => (diamond_points(p).to_vec(), true),
            Arrowhead::Square => (square_points(p).to_vec(), true),
            Arrowhead::Bar => (bar_points(p).to_vec(), false),
        };
        Some(Box::new(Polyline2d::new(pts, closed, closed)))
    }
}

fn polygon(points: &[PVec], closed: bool) -> PathBuilder {
    let mut path = PathBuilder::line_through_points(points);
    if closed {
        path.close();
    }
    path
}

fn open_arrow_points(p: HeadPoints) -> [PVec; 3] {
    [
        PVec::rot_with(p.int, p.point, PI / 6.0),
        p.point,
        PVec::rot_with(p.int, p.point, -PI / 6.0),
    ]
}

fn triangle_points(p: HeadPoints) -> [PVec; 3] {
    [
        PVec::rot_with(p.int, p.point, PI / 6.0),
        PVec::rot_with(p.int, p.point, -PI / 6.0),
        p.point,
    ]
}

fn inverted_points(p: HeadPoints) -> [PVec; 3] {
    let d = PVec::div(PVec::sub(p.int, p.point), 2.0);
    [
        PVec::add(p.point, PVec::rot(d, HALF_PI)),
        p.int,
        PVec::sub(p.point, PVec::rot(d, HALF_PI)),
    ]
}

fn diamond_points(p: HeadPoints) -> [PVec; 4] {
    let pb = PVec::lrp(p.point, p.int, 0.75);
    let pl = PVec::rot_with(pb, p.point, PI / 4.0);
    let pr = PVec::rot_with(pb, p.point, -PI / 4.0);
    let mid = PVec::lrp(pl, pr, 0.5);
    let pq = PVec::add(mid, PVec::sub(mid, p.point));
    [pq, pr, p.point, pl]
}

fn square_points(p: HeadPoints) -> [PVec; 4] {
    let pb = PVec::lrp(p.point, p.int, 0.85);
    let d = PVec::div(PVec::sub(pb, p.point), 2.0);
    let side = PVec::rot(d, HALF_PI);
    [
        PVec::add(p.point, side),
        PVec::add(pb, side),
        PVec::sub(pb, side),
        PVec::sub(p.point, side),
    ]
}

fn bar_points(p: HeadPoints) -> [PVec; 2] {
    let d = PVec::div(PVec::sub(p.int, p.point), 2.0);
    [
        PVec::add(p.point, PVec::rot(d, HALF_PI)),
        PVec::sub(p.point, PVec::rot(d, HALF_PI)),
    ]
}

fn open_arrow(p: HeadPoints) -> PathBuilder {
    polygon(&open_arrow_points(p), false)
}

fn triangle(p: HeadPoints) -> PathBuilder {
    polygon(&triangle_points(p), true)
}

fn inverted(p: HeadPoints) -> PathBuilder {
    polygon(&inverted_points(p), true)
}

fn diamond(p: HeadPoints) -> PathBuilder {
    polygon(&diamond_points(p), true)
}

fn square(p: HeadPoints) -> PathBuilder {
    polygon(&square_points(p), true)
}

fn bar(p: HeadPoints) -> PathBuilder {
    polygon(&bar_points(p), false)
}

fn dot(p: HeadPoints) -> PathBuilder {
    let a = PVec::lrp(p.point, p.int, 0.45);
    let r = PVec::dist(a, p.point);
    let mut path = PathBuilder::new();
    path.move_to(PVec::new(a.x - r, a.y));
    path.circular_arc(r, true, false, PVec::new(a.x + r, a.y));
    path.circular_arc(r, true, false, PVec::new(a.x - r, a.y));
    path
}
