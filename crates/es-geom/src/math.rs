use crate::vec::PVec;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub const HALF_PI: f64 = FRAC_PI_2;
pub const PI2: f64 = TAU;
pub const ARC_VERTEX_SPACING: f64 = 20.0;

/// The lower bound wins when `min > max`, unlike `f64::clamp`, which panics.
pub fn clamp(n: f64, min: f64, max: f64) -> f64 {
    min.max(n.min(max))
}

pub fn clamp_min(n: f64, min: f64) -> f64 {
    min.max(n)
}

pub fn modulate(value: f64, range_a: (f64, f64), range_b: (f64, f64), should_clamp: bool) -> f64 {
    let (from_low, from_high) = range_a;
    let (v0, v1) = range_b;
    let result = v0 + ((value - from_low) / (from_high - from_low)) * (v1 - v0);
    if !should_clamp {
        return result;
    }
    if v0 < v1 {
        result.min(v1).max(v0)
    } else {
        result.min(v0).max(v1)
    }
}

pub fn approximately(a: f64, b: f64, precision: f64) -> bool {
    (a - b).abs() <= precision
}

pub fn is_safe_float(n: f64) -> bool {
    n.is_finite() && n.abs() < 9_007_199_254_740_991.0
}

/// Ramanujan's approximation.
pub fn perimeter_of_ellipse(rx: f64, ry: f64) -> f64 {
    let h = (rx - ry).powi(2) / (rx + ry).powi(2);
    PI * (rx + ry) * (1.0 + (3.0 * h) / (10.0 + (4.0 - 3.0 * h).sqrt()))
}

/// A number in `[0, 2π)`.
pub fn canonicalize_rotation(a: f64) -> f64 {
    let mut a = a % PI2;
    if a < 0.0 {
        a += PI2;
    } else if a == 0.0 {
        a = 0.0;
    }
    a
}

pub fn clockwise_angle_dist(a0: f64, a1: f64) -> f64 {
    let a0 = canonicalize_rotation(a0);
    let mut a1 = canonicalize_rotation(a1);
    if a0 > a1 {
        a1 += PI2;
    }
    a1 - a0
}

pub fn counter_clockwise_angle_dist(a0: f64, a1: f64) -> f64 {
    PI2 - clockwise_angle_dist(a0, a1)
}

pub fn short_angle_dist(a0: f64, a1: f64) -> f64 {
    let da = (a1 - a0) % PI2;
    (2.0 * da) % PI2 - da
}

pub fn snap_angle(r: f64, segments: u32) -> f64 {
    let seg = PI2 / f64::from(segments);
    let mut ang = ((canonicalize_rotation(r) + seg / 2.0) / seg).floor() * seg % PI2;
    if ang < PI {
        ang += PI2;
    }
    if ang > PI {
        ang -= PI2;
    }
    ang
}

pub fn point_on_circle(center: PVec, r: f64, a: f64) -> PVec {
    PVec::add(center, PVec::from_angle(a, r))
}

/// Winding-number test; a point equal to a vertex counts as inside.
pub fn point_in_polygon(point: PVec, points: &[PVec]) -> bool {
    let n = points.len();
    if n == 0 {
        return false;
    }
    let mut winding = 0i32;
    for i in 0..n {
        let a = points[i];
        if a.x == point.x && a.y == point.y {
            return true;
        }
        let b = points[(i + 1) % n];
        let cross = (b.x - a.x) * (point.y - a.y) - (point.x - a.x) * (b.y - a.y);
        if a.y <= point.y {
            if b.y > point.y && cross > 0.0 {
                winding += 1;
            }
        } else if b.y <= point.y && cross < 0.0 {
            winding -= 1;
        }
    }
    winding != 0
}

/// `None` when the three points are collinear.
pub fn center_of_circle_from_three_points(a: PVec, b: PVec, c: PVec) -> Option<PVec> {
    let u = -2.0 * (a.x * (b.y - c.y) - a.y * (b.x - c.x) + b.x * c.y - c.x * b.y);
    let sa = a.x * a.x + a.y * a.y;
    let sb = b.x * b.x + b.y * b.y;
    let sc = c.x * c.x + c.y * c.y;
    let x = (sa * (c.y - b.y) + sb * (a.y - c.y) + sc * (b.y - a.y)) / u;
    let y = (sa * (b.x - c.x) + sb * (c.x - a.x) + sc * (a.x - b.x)) / u;
    (x.is_finite() && y.is_finite()).then(|| PVec::new(x, y))
}

/// The measure of an arc, negative when counter-clockwise.
pub fn arc_measure(a: f64, b: f64, sweep: bool, large_arc: bool) -> f64 {
    let diff = (b - a) % PI2;
    let m = (2.0 * diff) % PI2 - diff;
    if !large_arc {
        return m;
    }
    (PI2 - m.abs()) * if sweep { 1.0 } else { -1.0 }
}

/// Where along an arc an angle falls: 0 at the start, 1 at the end.
pub fn point_in_arc_t(m_ab: f64, a: f64, b: f64, p: f64) -> f64 {
    if m_ab.abs() > PI {
        let m_ap = short_angle_dist(a, p);
        let m_pb = short_angle_dist(p, b);
        if m_ap.abs() < m_pb.abs() {
            return m_ap / m_ab;
        }
        return (m_ab - m_pb) / m_ab;
    }
    let m_ap = short_angle_dist(a, p);
    let t = m_ap / m_ab;
    // An arc from -2.8 to 2.2 measures negative to its middle but positive near its ends; snap to the closer end.
    if (m_ap < 0.0) != (m_ab < 0.0) {
        return if t.abs() > 0.5 { 1.0 } else { 0.0 };
    }
    t
}

pub fn vertices_count_for_arc_length(length: f64, spacing: f64) -> usize {
    8.max((length / spacing).ceil() as usize)
}

/// Seeded xorshift over the seed's UTF-16 code units with signed 32-bit wrap semantics; values fall in about [-1, 1).
#[derive(Debug, Clone)]
pub struct Rng {
    x: i32,
    y: i32,
    z: i32,
    w: i32,
}

impl Rng {
    pub fn new(seed: &str) -> Self {
        let mut rng = Rng {
            x: 0,
            y: 0,
            z: 0,
            w: 0,
        };
        let units: Vec<u16> = seed.encode_utf16().collect();
        for k in 0..units.len() + 64 {
            rng.x ^= units.get(k).map_or(0, |u| i32::from(*u));
            rng.next();
        }
        rng
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        let t = self.x ^ self.x.wrapping_shl(11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        let shifted_w = ((self.w as u32) >> 19) as i32;
        let shifted_t = ((t as u32) >> 8) as i32;
        self.w ^= shifted_w ^ t ^ shifted_t;
        (f64::from(self.w) / 4_294_967_296.0) * 2.0
    }
}

pub mod easing {
    use std::f64::consts::PI;

    pub fn linear(t: f64) -> f64 {
        t
    }

    pub fn ease_out_quad(t: f64) -> f64 {
        t * (2.0 - t)
    }

    pub fn ease_out_cubic(t: f64) -> f64 {
        let u = t - 1.0;
        u * u * u + 1.0
    }

    pub fn ease_in_out_cubic(t: f64) -> f64 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            (t - 1.0) * (2.0 * t - 2.0) * (2.0 * t - 2.0) + 1.0
        }
    }

    pub fn ease_out_sine(t: f64) -> f64 {
        (t * PI / 2.0).sin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn rng_matches_signed_32_bit_reference_sequence() {
        let mut r = Rng::new("abc");
        assert!(close(r.next(), 0.65441347565501928));
        assert!(close(r.next(), 0.82886664802208543));
        assert!(close(r.next(), 0.83457690896466374));
        let mut r = Rng::new("arrow-10");
        assert!(close(r.next(), -0.38423486752435565));
        assert!(close(r.next(), 0.79155733296647668));
        assert!(close(r.next(), 0.52144277142360806));
    }

    #[test]
    fn rng_goes_negative_because_the_wrapped_state_is_signed() {
        let mut r = Rng::new("arrow-10");
        assert!(r.next() < 0.0);
    }

    #[test]
    fn rng_hashes_utf16_units_not_bytes() {
        let mut r = Rng::new("h\u{e9}llo");
        assert!(close(r.next(), 0.43415852962061763));
        assert!(close(r.next(), -0.30656603537499905));
    }

    #[test]
    fn rng_empty_seed_stays_at_the_all_zero_state() {
        let mut r = Rng::new("");
        assert_eq!(r.next(), 0.0);
        assert_eq!(r.next(), 0.0);
    }

    #[test]
    fn rng_is_reproducible_and_seed_dependent() {
        let mut a = Rng::new("shape-1");
        let mut b = Rng::new("shape-1");
        let mut c = Rng::new("shape-2");
        let (va, vb, vc) = (a.next(), b.next(), c.next());
        assert_eq!(va, vb);
        assert_ne!(va, vc);
    }

    #[test]
    fn canonical_rotation_wraps_into_zero_to_two_pi() {
        assert!(close(canonicalize_rotation(-FRAC_PI_2), 3.0 * FRAC_PI_2));
        assert!(close(canonicalize_rotation(5.0 * PI), PI));
        assert!(canonicalize_rotation(-0.0).is_sign_positive());
    }

    #[test]
    fn angle_distances_pick_the_expected_direction() {
        assert!(close(clockwise_angle_dist(0.0, FRAC_PI_2), FRAC_PI_2));
        assert!(close(clockwise_angle_dist(FRAC_PI_2, 0.0), 3.0 * FRAC_PI_2));
        assert!(close(
            counter_clockwise_angle_dist(0.0, FRAC_PI_2),
            3.0 * FRAC_PI_2
        ));
        assert!(close(short_angle_dist(0.1, PI2 - 0.1), -0.2));
    }

    #[test]
    fn snap_angle_rounds_to_the_nearest_segment() {
        assert!(close(snap_angle(0.1, 8), 0.0));
        assert!(close(snap_angle(FRAC_PI_2 + 0.1, 8), FRAC_PI_2));
        assert!(close(snap_angle(PI + 0.01, 8).abs(), PI));
    }

    #[test]
    fn modulate_maps_and_optionally_clamps_in_either_direction() {
        assert!(close(modulate(0.5, (0.0, 1.0), (10.0, 20.0), false), 15.0));
        assert!(close(modulate(2.0, (0.0, 1.0), (10.0, 20.0), true), 20.0));
        assert!(close(modulate(2.0, (0.0, 1.0), (10.0, 0.0), true), 0.0));
        assert!(close(modulate(-1.0, (0.0, 1.0), (10.0, 0.0), true), 10.0));
    }

    #[test]
    fn clamp_prefers_the_lower_bound_when_inverted() {
        assert_eq!(clamp(5.0, 10.0, 2.0), 10.0);
        assert_eq!(clamp(5.0, 0.0, 2.0), 2.0);
    }

    #[test]
    fn point_in_polygon_counts_vertices_and_rejects_outside() {
        let sq = [
            PVec::new(0.0, 0.0),
            PVec::new(4.0, 0.0),
            PVec::new(4.0, 4.0),
            PVec::new(0.0, 4.0),
        ];
        assert!(point_in_polygon(PVec::new(2.0, 2.0), &sq));
        assert!(point_in_polygon(PVec::new(4.0, 4.0), &sq));
        assert!(!point_in_polygon(PVec::new(5.0, 2.0), &sq));
        assert!(!point_in_polygon(PVec::new(2.0, 2.0), &[]));
    }

    #[test]
    fn circle_from_three_points_and_collinear_case() {
        let c = center_of_circle_from_three_points(
            PVec::new(1.0, 0.0),
            PVec::new(0.0, 1.0),
            PVec::new(-1.0, 0.0),
        );
        let c = c.unwrap();
        assert!(c.x.abs() < 1e-12 && c.y.abs() < 1e-12);
        assert!(
            center_of_circle_from_three_points(
                PVec::new(0.0, 0.0),
                PVec::new(1.0, 1.0),
                PVec::new(2.0, 2.0)
            )
            .is_none()
        );
    }

    #[test]
    fn arc_measure_sign_follows_sweep_for_large_arcs() {
        assert!(close(arc_measure(0.0, FRAC_PI_2, false, false), FRAC_PI_2));
        assert!(close(
            arc_measure(0.0, FRAC_PI_2, true, true),
            3.0 * FRAC_PI_2
        ));
        assert!(close(
            arc_measure(0.0, FRAC_PI_2, false, true),
            -3.0 * FRAC_PI_2
        ));
    }

    #[test]
    fn point_in_arc_t_snaps_to_the_nearer_end_on_the_far_side_of_the_circle() {
        let m = FRAC_PI_2;
        assert!(close(point_in_arc_t(m, 0.0, m, FRAC_PI_2 / 2.0), 0.5));
        assert_eq!(point_in_arc_t(m, 0.0, m, -0.1), 0.0);
        assert_eq!(point_in_arc_t(m, 0.0, m, PI), 1.0);
        assert!(point_in_arc_t(m, 0.0, m, FRAC_PI_2 + 0.5) > 1.0);
    }

    #[test]
    fn arc_vertex_count_has_a_floor_of_eight() {
        assert_eq!(vertices_count_for_arc_length(10.0, ARC_VERTEX_SPACING), 8);
        assert_eq!(vertices_count_for_arc_length(401.0, ARC_VERTEX_SPACING), 21);
    }

    #[test]
    fn ellipse_perimeter_of_a_circle_is_the_circumference() {
        assert!(close(perimeter_of_ellipse(5.0, 5.0), PI2 * 5.0));
    }
}
