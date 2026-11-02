use crate::vec::PVec;

pub const SEGMENT_PRECISION: f64 = 1e-10;

pub fn line_segment_line_segment(
    a1: PVec,
    a2: PVec,
    b1: PVec,
    b2: PVec,
    precision: f64,
) -> Option<PVec> {
    let abx = a1.x - b1.x;
    let aby = a1.y - b1.y;
    let bvx = b2.x - b1.x;
    let bvy = b2.y - b1.y;
    let avx = a2.x - a1.x;
    let avy = a2.y - a1.y;
    let ua_t = bvx * aby - bvy * abx;
    let ub_t = avx * aby - avy * abx;
    let u_b = bvy * avx - bvx * avy;

    if ua_t.abs() <= precision || ub_t.abs() <= precision {
        return None;
    }
    if u_b.abs() <= precision {
        return None;
    }

    let ua = ua_t / u_b;
    let ub = ub_t / u_b;
    let range = -precision..=1.0 + precision;
    if range.contains(&ua) && range.contains(&ub) {
        return Some(PVec::new(a1.x + ua * avx, a1.y + ua * avy));
    }
    None
}

pub fn line_segment_circle(a1: PVec, a2: PVec, c: PVec, r: f64) -> Vec<PVec> {
    let dx = a2.x - a1.x;
    let dy = a2.y - a1.y;
    let ocx = a1.x - c.x;
    let ocy = a1.y - c.y;

    let a = dx * dx + dy * dy;
    let b = 2.0 * (dx * ocx + dy * ocy);
    let cc = ocx * ocx + ocy * ocy - r * r;
    let deter = b * b - 4.0 * a * cc;

    if deter <= 0.0 {
        return Vec::new();
    }

    let e = deter.sqrt();
    let u1 = (-b + e) / (2.0 * a);
    let u2 = (-b - e) / (2.0 * a);

    let mut result = Vec::new();
    for u in [u1, u2] {
        if (0.0..=1.0).contains(&u) {
            result.push(PVec::new(a1.x + dx * u, a1.y + dy * u));
        }
    }
    result
}

pub fn line_segment_polyline(a1: PVec, a2: PVec, points: &[PVec]) -> Vec<PVec> {
    points
        .windows(2)
        .filter_map(|w| line_segment_line_segment(a1, a2, w[0], w[1], SEGMENT_PRECISION))
        .collect()
}

pub fn line_segment_polygon(a1: PVec, a2: PVec, points: &[PVec]) -> Vec<PVec> {
    let n = points.len();
    (0..n)
        .filter_map(|i| {
            line_segment_line_segment(a1, a2, points[i], points[(i + 1) % n], SEGMENT_PRECISION)
        })
        .collect()
}

/// Concentric, disjoint and contained circles yield NaN points rather than an empty list.
pub fn circle_circle(c1: PVec, r1: f64, c2: PVec, r2: f64) -> [PVec; 2] {
    let mut dx = c2.x - c1.x;
    let mut dy = c2.y - c1.y;
    let d = (dx * dx + dy * dy).sqrt();
    let x = (d * d - r2 * r2 + r1 * r1) / (2.0 * d);
    let y = (r1 * r1 - x * x).sqrt();
    dx /= d;
    dy /= d;
    [
        PVec::new(c1.x + dx * x - dy * y, c1.y + dy * x + dx * y),
        PVec::new(c1.x + dx * x + dy * y, c1.y + dy * x - dx * y),
    ]
}

pub fn circle_polygon(c: PVec, r: f64, points: &[PVec]) -> Vec<PVec> {
    let n = points.len();
    (0..n)
        .flat_map(|i| line_segment_circle(points[i], points[(i + 1) % n], c, r))
        .collect()
}

pub fn circle_polyline(c: PVec, r: f64, points: &[PVec]) -> Vec<PVec> {
    points
        .windows(2)
        .flat_map(|w| line_segment_circle(w[0], w[1], c, r))
        .collect()
}

fn ccw(a: PVec, b: PVec, c: PVec) -> bool {
    (c.y - a.y) * (b.x - a.x) > (b.y - a.y) * (c.x - a.x)
}

/// Proper crossing only: segments that touch at an endpoint or lie collinear are not counted.
pub fn lines_intersect(a: PVec, b: PVec, c: PVec, d: PVec) -> bool {
    ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
}

pub fn polygons_intersect(a: &[PVec], b: &[PVec]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    (0..a.len()).any(|i| {
        let (a0, a1) = (a[i], a[(i + 1) % a.len()]);
        (0..b.len()).any(|j| lines_intersect(a0, a1, b[j], b[(j + 1) % b.len()]))
    })
}

pub fn polygon_intersects_polyline(polygon: &[PVec], polyline: &[PVec]) -> bool {
    if polygon.is_empty() || polyline.len() < 2 {
        return false;
    }
    (0..polygon.len()).any(|i| {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        polyline
            .windows(2)
            .any(|w| lines_intersect(a, b, w[0], w[1]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> PVec {
        PVec::new(x, y)
    }

    fn seg(a: PVec, b: PVec, c: PVec, d: PVec) -> Option<PVec> {
        line_segment_line_segment(a, b, c, d, SEGMENT_PRECISION)
    }

    #[test]
    fn crossing_segments_meet_at_the_crossing() {
        let r = seg(p(0.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(4.0, 0.0)).unwrap();
        assert!((r.x - 2.0).abs() < 1e-12 && (r.y - 2.0).abs() < 1e-12);
    }

    #[test]
    fn parallel_and_collinear_segments_do_not_intersect() {
        assert!(seg(p(0.0, 0.0), p(4.0, 0.0), p(0.0, 1.0), p(4.0, 1.0)).is_none());
        assert!(seg(p(0.0, 0.0), p(4.0, 0.0), p(2.0, 0.0), p(6.0, 0.0)).is_none());
    }

    #[test]
    fn touching_the_other_segment_at_an_endpoint_is_rejected_as_coincident() {
        assert!(seg(p(0.0, 0.0), p(4.0, 0.0), p(2.0, 0.0), p(2.0, 3.0)).is_none());
    }

    #[test]
    fn segments_that_would_cross_beyond_their_ends_do_not_intersect() {
        assert!(seg(p(0.0, 0.0), p(1.0, 1.0), p(0.0, 4.0), p(4.0, 0.0)).is_none());
    }

    #[test]
    fn t_junction_in_the_interior_is_found() {
        let r = seg(p(0.0, 1.0), p(4.0, 1.0), p(2.0, 0.0), p(2.0, 3.0)).unwrap();
        assert!((r.x - 2.0).abs() < 1e-12 && (r.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn segment_through_a_circle_gives_two_points_ordered_far_then_near() {
        let r = line_segment_circle(p(-5.0, 0.0), p(5.0, 0.0), p(0.0, 0.0), 2.0);
        assert_eq!(r.len(), 2);
        assert!((r[0].x - 2.0).abs() < 1e-12 && (r[1].x + 2.0).abs() < 1e-12);
    }

    #[test]
    fn segment_ending_inside_a_circle_gives_one_point() {
        let r = line_segment_circle(p(-5.0, 0.0), p(0.0, 0.0), p(0.0, 0.0), 2.0);
        assert_eq!(r.len(), 1);
        assert!((r[0].x + 2.0).abs() < 1e-12);
    }

    #[test]
    fn tangent_missing_and_contained_segments_give_nothing() {
        let c = p(0.0, 0.0);
        assert!(line_segment_circle(p(-5.0, 2.0), p(5.0, 2.0), c, 2.0).is_empty());
        assert!(line_segment_circle(p(-5.0, 9.0), p(5.0, 9.0), c, 2.0).is_empty());
        assert!(line_segment_circle(p(-1.0, 0.0), p(1.0, 0.0), c, 2.0).is_empty());
        assert!(line_segment_circle(p(-5.0, 0.0), p(-4.0, 0.0), c, 2.0).is_empty());
    }

    #[test]
    fn polygon_crossing_collects_one_hit_per_edge_and_polyline_skips_the_closing_edge() {
        let sq = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let through = line_segment_polygon(p(-1.0, 2.0), p(5.0, 2.0), &sq);
        assert_eq!(through.len(), 2);
        let open = line_segment_polyline(p(-1.0, 2.0), p(5.0, 2.0), &sq);
        assert_eq!(open.len(), 1);
        assert!(line_segment_polyline(p(0.0, 0.0), p(1.0, 1.0), &sq[..1]).is_empty());
        assert!(line_segment_polygon(p(0.0, 0.0), p(1.0, 1.0), &[]).is_empty());
    }

    #[test]
    fn circle_polygon_and_polyline_gather_edge_hits() {
        let sq = [p(-3.0, -3.0), p(3.0, -3.0), p(3.0, 3.0), p(-3.0, 3.0)];
        assert_eq!(circle_polygon(p(0.0, 0.0), 3.0, &sq).len(), 0);
        assert_eq!(circle_polygon(p(0.0, 0.0), 4.0, &sq).len(), 8);
        assert_eq!(circle_polyline(p(0.0, 0.0), 4.0, &sq).len(), 6);
        assert!(circle_polyline(p(0.0, 0.0), 4.0, &sq[..1]).is_empty());
    }

    #[test]
    fn overlapping_circles_meet_at_two_symmetric_points() {
        let [a, b] = circle_circle(p(0.0, 0.0), 5.0, p(6.0, 0.0), 5.0);
        assert!((a.x - 3.0).abs() < 1e-12 && (a.y - 4.0).abs() < 1e-12);
        assert!((b.x - 3.0).abs() < 1e-12 && (b.y + 4.0).abs() < 1e-12);
    }

    #[test]
    fn disjoint_circles_give_nan_points() {
        let [a, _] = circle_circle(p(0.0, 0.0), 1.0, p(10.0, 0.0), 1.0);
        assert!(a.is_nan());
    }

    #[test]
    fn lines_intersect_counts_crossings_but_not_endpoint_touches() {
        assert!(lines_intersect(
            p(0.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(4.0, 0.0)
        ));
        assert!(!lines_intersect(
            p(0.0, 0.0),
            p(1.0, 1.0),
            p(1.0, 1.0),
            p(2.0, 0.0)
        ));
        assert!(!lines_intersect(
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(0.0, 1.0),
            p(4.0, 1.0)
        ));
    }

    #[test]
    fn nested_polygons_do_not_cross_but_overlapping_ones_do() {
        let big = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
        let inner = [p(2.0, 2.0), p(4.0, 2.0), p(4.0, 4.0), p(2.0, 4.0)];
        let shifted = [p(8.0, 8.0), p(12.0, 8.0), p(12.0, 12.0), p(8.0, 12.0)];
        assert!(!polygons_intersect(&big, &inner));
        assert!(polygons_intersect(&big, &shifted));
        assert!(!polygons_intersect(&[], &big));
    }

    #[test]
    fn polygon_polyline_crossing_needs_two_polyline_points() {
        let sq = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        assert!(polygon_intersects_polyline(
            &sq,
            &[p(-1.0, 2.0), p(2.0, 2.0)]
        ));
        assert!(!polygon_intersects_polyline(
            &sq,
            &[p(1.0, 1.0), p(2.0, 2.0)]
        ));
        assert!(!polygon_intersects_polyline(&sq, &[p(-1.0, 2.0)]));
    }
}
