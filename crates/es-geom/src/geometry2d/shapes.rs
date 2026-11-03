use super::Geometry2d;
use crate::bbox::BBox;
use crate::math::{
    ARC_VERTEX_SPACING, PI2, arc_measure, perimeter_of_ellipse, point_in_arc_t, point_in_polygon,
    point_on_circle, vertices_count_for_arc_length,
};
use crate::vec::PVec;

#[derive(Debug, Clone)]
pub struct Edge2d {
    pub start: PVec,
    pub end: PVec,
    vertices: [PVec; 2],
    dx: f64,
    dy: f64,
    len2: f64,
}

impl Edge2d {
    pub fn new(start: PVec, end: PVec) -> Self {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        Self {
            start,
            end,
            vertices: [start, end],
            dx,
            dy,
            len2: dx * dx + dy * dy,
        }
    }
}

impl Geometry2d for Edge2d {
    fn is_filled(&self) -> bool {
        false
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn length(&self) -> f64 {
        self.len2.sqrt()
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        if self.len2 == 0.0 {
            return self.start;
        }
        let t =
            ((point.x - self.start.x) * self.dx + (point.y - self.start.y) * self.dy) / self.len2;
        if t <= 0.0 {
            return self.start;
        }
        if t >= 1.0 {
            return self.end;
        }
        PVec::new(self.start.x + self.dx * t, self.start.y + self.dy * t)
    }

    fn distance_to_point(&self, point: PVec, _hit_inside: bool) -> f64 {
        PVec::dist(point, self.nearest_point(point))
    }
}

#[derive(Debug, Clone)]
pub struct Polyline2d {
    points: Vec<PVec>,
    closed: bool,
    filled: bool,
    bounds: BBox,
    segments: Vec<Edge2d>,
}

impl Polyline2d {
    pub fn new(points: Vec<PVec>, closed: bool, filled: bool) -> Self {
        let bounds = BBox::from_points(&points);
        Self::with_bounds(points, closed, filled, bounds)
    }

    pub fn polyline(points: Vec<PVec>) -> Self {
        Self::new(points, false, false)
    }

    pub fn polygon(points: Vec<PVec>, filled: bool) -> Self {
        Self::new(points, true, filled)
    }

    pub fn rectangle(x: f64, y: f64, width: f64, height: f64, filled: bool) -> Self {
        let points = vec![
            PVec::new(x, y),
            PVec::new(x + width, y),
            PVec::new(x + width, y + height),
            PVec::new(x, y + height),
        ];
        Self::with_bounds(points, true, filled, BBox::new(x, y, width, height))
    }

    fn with_bounds(points: Vec<PVec>, closed: bool, filled: bool, bounds: BBox) -> Self {
        let mut segments = Vec::new();
        if points.len() > 1 {
            segments.extend(points.windows(2).map(|w| Edge2d::new(w[0], w[1])));
            if closed {
                segments.push(Edge2d::new(points[points.len() - 1], points[0]));
            }
        }
        Self {
            points,
            closed,
            filled,
            bounds,
            segments,
        }
    }

    pub fn segments(&self) -> &[Edge2d] {
        &self.segments
    }
}

impl Geometry2d for Polyline2d {
    fn is_filled(&self) -> bool {
        self.filled
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn vertices(&self) -> &[PVec] {
        &self.points
    }

    fn bounds(&self) -> BBox {
        self.bounds
    }

    fn length(&self) -> f64 {
        self.segments.iter().map(|s| s.length()).sum()
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        let min = self
            .segments
            .iter()
            .map(|s| s.distance_to_point(point, false))
            .fold(f64::INFINITY, f64::min);
        if self.closed && (self.filled || hit_inside) && point_in_polygon(point, &self.points) {
            return -min;
        }
        min
    }

    fn hit_test_point(&self, point: PVec, margin: f64, hit_inside: bool) -> bool {
        self.distance_to_point(point, hit_inside) <= margin
    }
}

#[derive(Debug, Clone)]
pub struct Ellipse2d {
    w: f64,
    h: f64,
    filled: bool,
    vertices: Vec<PVec>,
}

impl Ellipse2d {
    pub fn new(width: f64, height: f64, filled: bool) -> Self {
        let w = width.max(1.0);
        let h = height.max(1.0);
        let cx = w / 2.0;
        let cy = h / 2.0;
        let perimeter = perimeter_of_ellipse(cx, cy);
        let count = vertices_count_for_arc_length(perimeter, ARC_VERTEX_SPACING);
        let step = PI2 / count as f64;

        let (b, a) = step.sin_cos();
        let (mut sinv, mut cosv) = (0.0f64, 1.0f64);
        let mut vertices = Vec::with_capacity(count);
        for _ in 0..count {
            vertices.push(PVec::new(
                (cx + cx * cosv).clamp(0.0, w),
                (cy + cy * sinv).clamp(0.0, h),
            ));
            let ts = b * cosv + a * sinv;
            let tc = a * cosv - b * sinv;
            sinv = ts;
            cosv = tc;
        }
        Self {
            w: width,
            h: height,
            filled,
            vertices,
        }
    }

    fn edges(&self) -> impl Iterator<Item = Edge2d> + '_ {
        let n = self.vertices.len();
        (0..n).map(move |i| Edge2d::new(self.vertices[i], self.vertices[(i + 1) % n]))
    }
}

impl Geometry2d for Ellipse2d {
    fn is_filled(&self) -> bool {
        self.filled
    }

    fn is_closed(&self) -> bool {
        true
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn bounds(&self) -> BBox {
        BBox::new(0.0, 0.0, self.w, self.h)
    }

    fn length(&self) -> f64 {
        perimeter_of_ellipse((self.w / 2.0).max(0.0), (self.h / 2.0).max(0.0))
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        let mut nearest = point;
        let mut dist = f64::INFINITY;
        for edge in self.edges() {
            let p = edge.nearest_point(point);
            let d = PVec::dist2(p, point);
            if d < dist {
                nearest = p;
                dist = d;
            }
        }
        nearest
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        let min = self
            .edges()
            .map(|e| e.distance_to_point(point, false))
            .fold(f64::INFINITY, f64::min);
        if (self.filled || hit_inside) && point_in_polygon(point, &self.vertices) {
            return -min;
        }
        min
    }
}

#[derive(Debug, Clone)]
pub struct Circle2d {
    x: f64,
    y: f64,
    radius: f64,
    center: PVec,
    filled: bool,
    vertices: Vec<PVec>,
}

impl Circle2d {
    pub fn new(x: f64, y: f64, radius: f64, filled: bool) -> Self {
        let center = PVec::new(radius + x, radius + y);
        let n = vertices_count_for_arc_length(PI2 * radius, ARC_VERTEX_SPACING);
        let vertices = (0..n)
            .map(|i| point_on_circle(center, radius, (i as f64 / n as f64) * PI2))
            .collect();
        Self {
            x,
            y,
            radius,
            center,
            filled,
            vertices,
        }
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }
}

impl Geometry2d for Circle2d {
    fn is_filled(&self) -> bool {
        self.filled
    }

    fn is_closed(&self) -> bool {
        true
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn bounds(&self) -> BBox {
        BBox::new(self.x, self.y, self.radius * 2.0, self.radius * 2.0)
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        let dx = point.x - self.center.x;
        let dy = point.y - self.center.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            return PVec::new(self.center.x + self.radius, self.center.y);
        }
        let scale = self.radius / len;
        PVec::new(self.center.x + dx * scale, self.center.y + dy * scale)
    }

    fn distance_to_point(&self, point: PVec, hit_inside: bool) -> f64 {
        let to_edge = PVec::dist(point, self.center) - self.radius;
        if to_edge < 0.0 && (self.filled || hit_inside) {
            return to_edge;
        }
        to_edge.abs()
    }

    fn hit_test_point(&self, point: PVec, margin: f64, hit_inside: bool) -> bool {
        let dist2 = PVec::dist2(point, self.center);
        if (self.filled || hit_inside) && dist2 <= self.radius * self.radius {
            return true;
        }
        let outer = self.radius + margin;
        if dist2 > outer * outer {
            return false;
        }
        let inner = self.radius - margin;
        if inner <= 0.0 {
            return true;
        }
        dist2 >= inner * inner
    }
}

#[derive(Debug, Clone)]
pub struct Arc2d {
    center: PVec,
    radius: f64,
    start: PVec,
    end: PVec,
    measure: f64,
    angle_start: f64,
    angle_end: f64,
    vertices: Vec<PVec>,
}

impl Arc2d {
    pub fn new(center: PVec, start: PVec, end: PVec, sweep: bool, large_arc: bool) -> Self {
        let angle_start = center.angle(start);
        let angle_end = center.angle(end);
        let radius = PVec::dist(center, start);
        let measure = arc_measure(angle_start, angle_end, sweep, large_arc);
        let n = vertices_count_for_arc_length((measure * radius).abs(), ARC_VERTEX_SPACING);
        let vertices = (0..=n)
            .map(|i| {
                let t = (i as f64 / n as f64) * measure;
                point_on_circle(center, radius, angle_start + t)
            })
            .collect();
        Self {
            center,
            radius,
            start,
            end,
            measure,
            angle_start,
            angle_end,
            vertices,
        }
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }

    pub fn measure(&self) -> f64 {
        self.measure
    }
}

impl Geometry2d for Arc2d {
    fn is_filled(&self) -> bool {
        false
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn vertices(&self) -> &[PVec] {
        &self.vertices
    }

    fn length(&self) -> f64 {
        (self.measure * self.radius).abs()
    }

    fn nearest_point(&self, point: PVec) -> PVec {
        let t = point_in_arc_t(
            self.measure,
            self.angle_start,
            self.angle_end,
            self.center.angle(point),
        );
        if t <= 0.0 {
            return self.start;
        }
        if t >= 1.0 {
            return self.end;
        }
        let dx = point.x - self.center.x;
        let dy = point.y - self.center.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            return self.start;
        }
        let scale = self.radius / len;
        PVec::new(self.center.x + dx * scale, self.center.y + dy * scale)
    }
}
