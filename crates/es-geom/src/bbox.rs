use crate::vec::PVec;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl BBox {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn min_x(self) -> f64 {
        self.x
    }
    pub fn min_y(self) -> f64 {
        self.y
    }
    pub fn max_x(self) -> f64 {
        self.x + self.w
    }
    pub fn max_y(self) -> f64 {
        self.y + self.h
    }
    pub fn mid_x(self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn mid_y(self) -> f64 {
        self.y + self.h / 2.0
    }

    pub fn point(self) -> PVec {
        PVec::new(self.x, self.y)
    }

    pub fn center(self) -> PVec {
        PVec::new(self.mid_x(), self.mid_y())
    }

    /// Corners in clockwise order from the top left.
    pub fn corners(self) -> [PVec; 4] {
        [
            PVec::new(self.min_x(), self.min_y()),
            PVec::new(self.max_x(), self.min_y()),
            PVec::new(self.max_x(), self.max_y()),
            PVec::new(self.min_x(), self.max_y()),
        ]
    }

    pub fn sides(self) -> [(PVec, PVec); 4] {
        let c = self.corners();
        [(c[0], c[1]), (c[1], c[2]), (c[2], c[3]), (c[3], c[0])]
    }

    /// An empty slice gives the zero box.
    pub fn from_points(points: &[PVec]) -> BBox {
        if points.is_empty() {
            return BBox::default();
        }
        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        BBox::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// The union of the boxes. An empty slice gives the zero box.
    pub fn common(boxes: &[BBox]) -> BBox {
        let Some(first) = boxes.first() else {
            return BBox::default();
        };
        let (mut min_x, mut min_y, mut max_x, mut max_y) =
            (first.min_x(), first.min_y(), first.max_x(), first.max_y());
        for b in &boxes[1..] {
            min_x = min_x.min(b.min_x());
            min_y = min_y.min(b.min_y());
            max_x = max_x.max(b.max_x());
            max_y = max_y.max(b.max_y());
        }
        BBox::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn contains_point(self, p: PVec, margin: f64) -> bool {
        !(p.x < self.min_x() - margin
            || p.y < self.min_y() - margin
            || p.x > self.max_x() + margin
            || p.y > self.max_y() + margin)
    }

    pub fn contains(self, other: BBox) -> bool {
        other.min_x() >= self.min_x()
            && other.min_y() >= self.min_y()
            && other.max_x() <= self.max_x()
            && other.max_y() <= self.max_y()
    }

    /// Touching edges count as colliding.
    pub fn collides(self, other: BBox) -> bool {
        !(other.max_x() < self.min_x()
            || other.min_x() > self.max_x()
            || other.max_y() < self.min_y()
            || other.min_y() > self.max_y())
    }

    pub fn expand_by(self, n: f64) -> BBox {
        BBox::new(self.x - n, self.y - n, self.w + n * 2.0, self.h + n * 2.0)
    }
}

impl From<BBox> for crate::Rect {
    fn from(b: BBox) -> Self {
        crate::Rect::new(b.min_x(), b.min_y(), b.max_x(), b.max_y())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_points_and_common_handle_empty_input() {
        assert_eq!(BBox::from_points(&[]), BBox::default());
        assert_eq!(BBox::common(&[]), BBox::default());
    }

    #[test]
    fn from_points_spans_extremes() {
        let b = BBox::from_points(&[PVec::new(3.0, -1.0), PVec::new(-2.0, 4.0), PVec::new(0.0, 0.0)]);
        assert_eq!(b, BBox::new(-2.0, -1.0, 5.0, 5.0));
    }

    #[test]
    fn common_is_the_union() {
        let u = BBox::common(&[BBox::new(0.0, 0.0, 2.0, 2.0), BBox::new(5.0, -3.0, 1.0, 1.0)]);
        assert_eq!(u, BBox::new(0.0, -3.0, 6.0, 5.0));
    }

    #[test]
    fn touching_boxes_collide_but_gapped_do_not() {
        let a = BBox::new(0.0, 0.0, 2.0, 2.0);
        assert!(a.collides(BBox::new(2.0, 0.0, 2.0, 2.0)));
        assert!(!a.collides(BBox::new(2.1, 0.0, 2.0, 2.0)));
    }

    #[test]
    fn contains_point_margin_extends_outward() {
        let a = BBox::new(0.0, 0.0, 2.0, 2.0);
        assert!(a.contains_point(PVec::new(2.0, 2.0), 0.0));
        assert!(!a.contains_point(PVec::new(2.5, 1.0), 0.0));
        assert!(a.contains_point(PVec::new(2.5, 1.0), 0.5));
    }

    #[test]
    fn corners_expand_and_contains() {
        let c = BBox::new(1.0, 2.0, 3.0, 4.0).corners();
        assert_eq!(c[0], PVec::new(1.0, 2.0));
        assert_eq!(c[2], PVec::new(4.0, 6.0));
        assert!(BBox::new(0.0, 0.0, 9.0, 9.0).contains(BBox::new(1.0, 1.0, 2.0, 2.0)));
        assert_eq!(BBox::new(0.0, 0.0, 2.0, 2.0).expand_by(1.0), BBox::new(-1.0, -1.0, 4.0, 4.0));
    }
}
