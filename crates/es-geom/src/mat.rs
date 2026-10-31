use crate::vec::PVec;
use std::f64::consts::FRAC_PI_2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decomposed {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation: f64,
}

impl Default for Mat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mat {
    pub const IDENTITY: Mat = Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub const fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    pub fn translate(x: f64, y: f64) -> Mat {
        Mat::new(1.0, 0.0, 0.0, 1.0, x, y)
    }

    pub fn rotate(r: f64) -> Mat {
        let (sinr, cosr) = r.sin_cos();
        Mat::new(cosr, sinr, -sinr, cosr, 0.0, 0.0)
    }

    pub fn scale(x: f64, y: f64) -> Mat {
        Mat::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    pub fn compose(x: f64, y: f64, rotation: f64) -> Mat {
        Mat::multiply(Mat::translate(x, y), Mat::rotate(rotation))
    }

    pub fn multiply(m1: Mat, m2: Mat) -> Mat {
        Mat::new(
            m1.a * m2.a + m1.c * m2.b,
            m1.b * m2.a + m1.d * m2.b,
            m1.a * m2.c + m1.c * m2.d,
            m1.b * m2.c + m1.d * m2.d,
            m1.a * m2.e + m1.c * m2.f + m1.e,
            m1.b * m2.e + m1.d * m2.f + m1.f,
        )
    }

    /// Singular matrices are not guarded: the result is non-finite.
    pub fn inverse(self) -> Mat {
        let denom = self.a * self.d - self.b * self.c;
        Mat::new(
            self.d / denom,
            self.b / -denom,
            self.c / -denom,
            self.a / denom,
            (self.d * self.e - self.c * self.f) / -denom,
            (self.b * self.e - self.a * self.f) / denom,
        )
    }

    pub fn apply_to_point(self, p: PVec) -> PVec {
        PVec::with_z(
            self.a * p.x + self.c * p.y + self.e,
            self.b * p.x + self.d * p.y + self.f,
            p.z,
        )
    }

    pub fn apply_to_points(self, points: &[PVec]) -> Vec<PVec> {
        points.iter().map(|p| self.apply_to_point(*p)).collect()
    }

    pub fn apply_inverse_to_point(self, p: PVec) -> PVec {
        self.inverse().apply_to_point(p)
    }

    /// Assumes no skew, which holds for shape transforms (translate + rotate + scale).
    pub fn decomposed(self) -> Decomposed {
        let Mat { a, b, c, d, e, f } = self;
        if a != 0.0 || b != 0.0 {
            let r = (a * a + b * b).sqrt();
            return Decomposed {
                x: e,
                y: f,
                scale_x: r,
                scale_y: (a * d - b * c) / r,
                rotation: (a / r).acos() * if b > 0.0 { 1.0 } else { -1.0 },
            };
        }
        if c != 0.0 || d != 0.0 {
            let s = (c * c + d * d).sqrt();
            return Decomposed {
                x: e,
                y: f,
                scale_x: (a * d - b * c) / s,
                scale_y: s,
                rotation: FRAC_PI_2 + (c / s).acos() * if d > 0.0 { 1.0 } else { -1.0 },
            };
        }
        Decomposed { x: e, y: f, scale_x: 0.0, scale_y: 0.0, rotation: 0.0 }
    }

    pub fn rotation(self) -> f64 {
        self.decomposed().rotation
    }
}

impl From<Mat> for kurbo::Affine {
    fn from(m: Mat) -> Self {
        kurbo::Affine::new([m.a, m.b, m.c, m.d, m.e, m.f])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: PVec, b: PVec) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn compose_rotates_about_shape_origin_then_translates() {
        let m = Mat::compose(10.0, 20.0, FRAC_PI_2);
        assert!(close(m.apply_to_point(PVec::new(1.0, 0.0)), PVec::new(10.0, 21.0)));
    }

    #[test]
    fn inverse_undoes_the_transform_and_keeps_pressure() {
        let m = Mat::multiply(Mat::compose(5.0, -3.0, 0.7), Mat::scale(2.0, 3.0));
        let p = PVec::with_z(4.0, 9.0, 0.25);
        let back = m.apply_inverse_to_point(m.apply_to_point(p));
        assert!(close(back, p));
        assert_eq!(back.z, 0.25);
    }

    #[test]
    fn decomposed_recovers_rotation_and_scale_in_both_branches() {
        let m = Mat::multiply(Mat::compose(7.0, 8.0, 0.6), Mat::scale(2.0, 3.0));
        let d = m.decomposed();
        assert!((d.rotation - 0.6).abs() < 1e-9);
        assert!((d.scale_x - 2.0).abs() < 1e-9 && (d.scale_y - 3.0).abs() < 1e-9);
        assert_eq!((d.x, d.y), (7.0, 8.0));

        let q = Mat::new(0.0, 0.0, 0.0, 2.0, 1.0, 1.0).decomposed();
        assert!((q.rotation - std::f64::consts::PI).abs() < 1e-9);
        assert_eq!((q.scale_x, q.scale_y), (0.0, 2.0));

        let z = Mat::new(0.0, 0.0, 0.0, 0.0, 3.0, 4.0).decomposed();
        assert_eq!((z.scale_x, z.scale_y, z.rotation), (0.0, 0.0, 0.0));
    }

    #[test]
    fn negative_b_gives_negative_rotation() {
        assert!((Mat::rotate(-0.4).rotation() + 0.4).abs() < 1e-9);
    }

    #[test]
    fn converts_to_kurbo_affine_with_same_mapping() {
        let m = Mat::compose(3.0, 4.0, 0.5);
        let p = m.apply_to_point(PVec::new(2.0, 1.0));
        let q = kurbo::Affine::from(m) * kurbo::Point::new(2.0, 1.0);
        assert!((p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
    }
}
