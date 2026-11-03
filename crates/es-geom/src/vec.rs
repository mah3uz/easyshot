use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PVec {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for PVec {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

#[allow(clippy::should_implement_trait)]
impl PVec {
    pub const ZERO: PVec = PVec {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// `z` defaults to 1.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y, z: 1.0 }
    }

    pub const fn with_z(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn len(self) -> f64 {
        self.len2().sqrt()
    }

    /// The unit vector. Zero-length vectors are returned unchanged.
    pub fn uni(self) -> PVec {
        let l = self.len();
        if l == 0.0 {
            return self;
        }
        PVec::with_z(self.x / l, self.y / l, self.z)
    }

    /// Perpendicular (rotated 90 degrees clockwise in a y-down space).
    pub fn per(self) -> PVec {
        PVec::with_z(self.y, -self.x, self.z)
    }

    pub fn is_nan(self) -> bool {
        self.x.is_nan() || self.y.is_nan()
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn angle(self, b: PVec) -> f64 {
        (b.y - self.y).atan2(b.x - self.x)
    }

    pub fn add(a: PVec, b: PVec) -> PVec {
        PVec::with_z(a.x + b.x, a.y + b.y, a.z)
    }

    pub fn add_xy(a: PVec, x: f64, y: f64) -> PVec {
        PVec::with_z(a.x + x, a.y + y, a.z)
    }

    pub fn sub(a: PVec, b: PVec) -> PVec {
        PVec::with_z(a.x - b.x, a.y - b.y, a.z)
    }

    pub fn sub_xy(a: PVec, x: f64, y: f64) -> PVec {
        PVec::with_z(a.x - x, a.y - y, a.z)
    }

    pub fn mul(a: PVec, s: f64) -> PVec {
        PVec::with_z(a.x * s, a.y * s, a.z)
    }

    pub fn mul_v(a: PVec, b: PVec) -> PVec {
        PVec::with_z(a.x * b.x, a.y * b.y, a.z)
    }

    pub fn div(a: PVec, s: f64) -> PVec {
        PVec::with_z(a.x / s, a.y / s, a.z)
    }

    /// Midpoint; `z` resets to 1.
    pub fn med(a: PVec, b: PVec) -> PVec {
        PVec::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    /// Linear interpolation; `z` resets to 1.
    pub fn lrp(a: PVec, b: PVec, t: f64) -> PVec {
        PVec::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }

    pub fn dist2(a: PVec, b: PVec) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dx * dx + dy * dy
    }

    pub fn dist(a: PVec, b: PVec) -> f64 {
        Self::dist2(a, b).sqrt()
    }

    pub fn dist_min(a: PVec, b: PVec, n: f64) -> bool {
        Self::dist2(a, b) < n * n
    }

    pub fn tan(a: PVec, b: PVec) -> PVec {
        Self::sub(a, b).uni()
    }

    pub fn nudge(a: PVec, b: PVec, distance: f64) -> PVec {
        Self::add(a, Self::mul(Self::tan(b, a), distance))
    }

    pub fn from_angle(radians: f64, length: f64) -> PVec {
        PVec::new(radians.cos() * length, radians.sin() * length)
    }

    pub fn angle_between(a: PVec, b: PVec) -> f64 {
        let p = a.x * b.x + a.y * b.y;
        let n = ((a.x * a.x + a.y * a.y) * (b.x * b.x + b.y * b.y)).sqrt();
        let sign = if a.x * b.y - a.y * b.x < 0.0 {
            -1.0
        } else {
            1.0
        };
        sign * (p / n).clamp(-1.0, 1.0).acos()
    }

    /// Rotate a vector about the origin. `z` resets to 1.
    pub fn rot(a: PVec, radians: f64) -> PVec {
        let (s, c) = radians.sin_cos();
        PVec::new(a.x * c - a.y * s, a.x * s + a.y * c)
    }

    /// Rotate `a` about `center`. `z` resets to 1 unless the angle is exactly 0.
    pub fn rot_with(a: PVec, center: PVec, radians: f64) -> PVec {
        if radians == 0.0 {
            return a;
        }
        let (s, c) = radians.sin_cos();
        let px = a.x - center.x;
        let py = a.y - center.y;
        PVec::new(px * c - py * s + center.x, px * s + py * c + center.y)
    }

    pub fn clockwise(a: PVec, b: PVec, c: PVec) -> bool {
        (c.x - a.x) * (b.y - a.y) - (b.x - a.x) * (c.y - a.y) < 0.0
    }

    pub fn average(points: &[PVec]) -> PVec {
        if points.is_empty() {
            return PVec::new(0.0, 0.0);
        }
        let (mut x, mut y) = (0.0, 0.0);
        for p in points {
            x += p.x;
            y += p.y;
        }
        let n = points.len() as f64;
        PVec::new(x / n, y / n)
    }

    pub fn nearest_point_on_line_segment(a: PVec, b: PVec, p: PVec, clamp: bool) -> PVec {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let d2 = dx * dx + dy * dy;
        if d2 == 0.0 {
            return a;
        }
        let mut t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / d2;
        if clamp {
            t = t.clamp(0.0, 1.0);
        }
        PVec::new(a.x + t * dx, a.y + t * dy)
    }

    /// Tolerance compare (1e-4 per axis, `z` ignored): an exact compare would report a flip on every bound arrow.
    pub fn equals(a: PVec, b: PVec) -> bool {
        (a.x - b.x).abs() < 0.0001 && (a.y - b.y).abs() < 0.0001
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn default_z_is_one() {
        assert_eq!(PVec::new(1.0, 2.0).z, 1.0);
    }

    #[test]
    fn uni_of_zero_vector_is_unchanged() {
        let v = PVec::with_z(0.0, 0.0, 0.3);
        assert_eq!(v.uni(), v);
        assert_eq!(PVec::new(3.0, 4.0).uni(), PVec::new(0.6, 0.8));
    }

    #[test]
    fn lrp_and_med_reset_pressure_but_add_keeps_it() {
        let a = PVec::with_z(0.0, 0.0, 0.2);
        let b = PVec::with_z(2.0, 2.0, 0.8);
        assert_eq!(PVec::lrp(a, b, 0.5).z, 1.0);
        assert_eq!(PVec::med(a, b).z, 1.0);
        assert_eq!(PVec::add(a, b).z, 0.2);
    }

    #[test]
    fn equals_uses_tolerance_not_exact() {
        let a = PVec::new(1.0, 1.0);
        assert!(PVec::equals(a, PVec::new(1.00005, 0.99995)));
        assert!(!PVec::equals(a, PVec::new(1.0002, 1.0)));
    }

    #[test]
    fn angle_between_is_signed_and_survives_rounding() {
        let x = PVec::new(1.0, 0.0);
        let y = PVec::new(0.0, 1.0);
        assert!((PVec::angle_between(x, y) - FRAC_PI_2).abs() < 1e-12);
        assert!((PVec::angle_between(y, x) + FRAC_PI_2).abs() < 1e-12);
        let v = PVec::new(0.1, 0.1);
        assert!(!PVec::angle_between(v, v).is_nan());
    }

    #[test]
    fn nearest_point_clamps_only_when_asked() {
        let a = PVec::new(0.0, 0.0);
        let b = PVec::new(10.0, 0.0);
        let p = PVec::new(20.0, 5.0);
        assert_eq!(
            PVec::nearest_point_on_line_segment(a, b, p, true),
            PVec::new(10.0, 0.0)
        );
        assert_eq!(
            PVec::nearest_point_on_line_segment(a, b, p, false),
            PVec::new(20.0, 0.0)
        );
        assert_eq!(PVec::nearest_point_on_line_segment(a, a, p, true), a);
    }

    #[test]
    fn rot_with_zero_angle_keeps_z_otherwise_resets() {
        let a = PVec::with_z(1.0, 0.0, 0.4);
        let c = PVec::ZERO;
        assert_eq!(PVec::rot_with(a, c, 0.0).z, 0.4);
        let r = PVec::rot_with(a, c, FRAC_PI_2);
        assert!(r.x.abs() < 1e-12 && (r.y - 1.0).abs() < 1e-12);
        assert_eq!(r.z, 1.0);
    }

    #[test]
    fn nudge_moves_towards_target() {
        let r = PVec::nudge(PVec::new(0.0, 0.0), PVec::new(10.0, 0.0), 3.0);
        assert_eq!(r, PVec::new(3.0, 0.0));
    }

    #[test]
    fn clockwise_matches_y_down_winding() {
        assert!(PVec::clockwise(
            PVec::new(0.0, 0.0),
            PVec::new(1.0, 0.0),
            PVec::new(1.0, 1.0)
        ));
    }
}
