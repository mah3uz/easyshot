use super::*;
use std::f64::consts::{FRAC_PI_2, PI};

fn p(x: f64, y: f64) -> PVec {
    PVec::new(x, y)
}

fn square(filled: bool) -> Polyline2d {
    Polyline2d::rectangle(0.0, 0.0, 10.0, 10.0, filled)
}

#[test]
fn outline_hit_test_accepts_up_to_the_margin_and_rejects_beyond() {
    let r = square(false);
    assert!(r.hit_test_point(p(10.0, 5.0), 0.0, false));
    assert!(r.hit_test_point(p(12.0, 5.0), 2.0, false));
    assert!(!r.hit_test_point(p(12.1, 5.0), 2.0, false));
    assert!(r.hit_test_point(p(8.0, 5.0), 2.0, false));
    assert!(!r.hit_test_point(p(7.9, 5.0), 2.0, false));
}

#[test]
fn only_filled_or_hit_inside_shapes_are_hit_in_their_interior() {
    let hollow = square(false);
    let solid = square(true);
    assert!(!hollow.hit_test_point(p(5.0, 5.0), 1.0, false));
    assert!(hollow.hit_test_point(p(5.0, 5.0), 1.0, true));
    assert!(solid.hit_test_point(p(5.0, 5.0), 1.0, false));
}

#[test]
fn distance_is_negative_inside_a_hit_inside_shape_only() {
    let hollow = square(false);
    assert!((hollow.distance_to_point(p(5.0, 2.0), false) - 2.0).abs() < 1e-12);
    assert!((hollow.distance_to_point(p(5.0, 2.0), true) + 2.0).abs() < 1e-12);
    assert!((square(true).distance_to_point(p(5.0, 12.0), false) - 2.0).abs() < 1e-12);
}

#[test]
fn rectangle_bounds_and_length_follow_its_box() {
    let r = Polyline2d::rectangle(2.0, 3.0, 10.0, 4.0, false);
    assert_eq!(r.bounds(), BBox::new(2.0, 3.0, 10.0, 4.0));
    assert!((r.length() - 28.0).abs() < 1e-12);
    assert_eq!(r.segments().len(), 4);
}

#[test]
fn open_polyline_has_no_closing_edge() {
    let l = Polyline2d::polyline(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
    assert_eq!(l.segments().len(), 2);
    assert!(!l.hit_test_point(p(0.0, 10.0), 1.0, false));
    assert!(l.hit_test_point(p(5.0, 1.0), 1.0, false));
}

#[test]
fn degenerate_polyline_never_hits() {
    let l = Polyline2d::polyline(vec![p(1.0, 1.0)]);
    assert!(!l.hit_test_point(p(1.0, 1.0), 5.0, false));
    assert_eq!(l.length(), 0.0);
}

#[test]
fn edge_clamps_to_its_endpoints_and_survives_zero_length() {
    let e = Edge2d::new(p(0.0, 0.0), p(10.0, 0.0));
    assert_eq!(e.nearest_point(p(15.0, 3.0)), p(10.0, 0.0));
    assert_eq!(e.nearest_point(p(-5.0, 3.0)), p(0.0, 0.0));
    assert_eq!(e.nearest_point(p(4.0, 3.0)), p(4.0, 0.0));
    assert_eq!(e.length(), 10.0);
    let z = Edge2d::new(p(1.0, 1.0), p(1.0, 1.0));
    assert_eq!(z.nearest_point(p(9.0, 9.0)), p(1.0, 1.0));
}

#[test]
fn edge_distance_ignores_hit_inside() {
    let e = Edge2d::new(p(0.0, 0.0), p(10.0, 0.0));
    assert_eq!(e.distance_to_point(p(5.0, 3.0), true), 3.0);
}

#[test]
fn ellipse_vertices_stay_in_its_box_and_hug_the_curve() {
    let e = Ellipse2d::new(100.0, 40.0, false);
    assert!(e.vertices().len() >= 8);
    for v in e.vertices() {
        assert!((0.0..=100.0).contains(&v.x) && (0.0..=40.0).contains(&v.y));
        let k = ((v.x - 50.0) / 50.0).powi(2) + ((v.y - 20.0) / 20.0).powi(2);
        assert!((k - 1.0).abs() < 1e-9);
    }
    assert_eq!(e.bounds(), BBox::new(0.0, 0.0, 100.0, 40.0));
}

#[test]
fn ellipse_vertex_count_follows_perimeter() {
    let small = Ellipse2d::new(10.0, 10.0, false);
    let big = Ellipse2d::new(400.0, 400.0, false);
    assert_eq!(small.vertices().len(), 8);
    assert_eq!(big.vertices().len(), (PI * 400.0 / 20.0).ceil() as usize);
}

#[test]
fn ellipse_hit_test_margin_straddles_the_outline() {
    let e = Ellipse2d::new(100.0, 100.0, false);
    assert!(e.hit_test_point(p(100.0, 50.0), 0.0, false));
    assert!(e.hit_test_point(p(104.0, 50.0), 4.0, false));
    assert!(!e.hit_test_point(p(105.0, 50.0), 4.0, false));
    assert!(!e.hit_test_point(p(50.0, 50.0), 4.0, false));
    assert!(Ellipse2d::new(100.0, 100.0, true).hit_test_point(p(50.0, 50.0), 4.0, false));
}

#[test]
fn ellipse_with_collapsed_size_is_clamped_to_one_for_vertices_only() {
    let e = Ellipse2d::new(0.0, 0.0, false);
    assert!(e.vertices().iter().all(|v| v.x <= 1.0 && v.y <= 1.0));
}

#[test]
fn circle_hit_test_has_inner_and_outer_margin_and_fills_when_small() {
    let c = Circle2d::new(0.0, 0.0, 10.0, false);
    let center = c.circle_center();
    assert_eq!(center, p(10.0, 10.0));
    assert!(c.hit_test_point(p(20.0, 10.0), 0.0, false));
    assert!(c.hit_test_point(p(23.0, 10.0), 3.0, false));
    assert!(!c.hit_test_point(p(23.1, 10.0), 3.0, false));
    assert!(c.hit_test_point(p(17.0, 10.0), 3.0, false));
    assert!(!c.hit_test_point(p(16.9, 10.0), 3.0, false));
    assert!(!c.hit_test_point(center, 3.0, false));
    assert!(c.hit_test_point(center, 11.0, false));
    assert!(Circle2d::new(0.0, 0.0, 10.0, true).hit_test_point(center, 0.0, false));
}

#[test]
fn circle_distance_is_signed_only_for_filled() {
    let hollow = Circle2d::new(0.0, 0.0, 10.0, false);
    let solid = Circle2d::new(0.0, 0.0, 10.0, true);
    assert_eq!(hollow.distance_to_point(p(10.0, 10.0), false), 10.0);
    assert_eq!(solid.distance_to_point(p(10.0, 10.0), false), -10.0);
    assert_eq!(solid.distance_to_point(p(25.0, 10.0), false), 5.0);
}

#[test]
fn circle_nearest_point_at_its_center_is_the_rightmost_point() {
    let c = Circle2d::new(0.0, 0.0, 10.0, false);
    assert_eq!(c.nearest_point(c.circle_center()), p(20.0, 10.0));
    assert_eq!(c.bounds(), BBox::new(0.0, 0.0, 20.0, 20.0));
}

fn quarter_arc() -> Arc2d {
    Arc2d::new(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), true, false)
}

#[test]
fn arc_measure_radius_and_length() {
    let a = quarter_arc();
    assert!((a.measure() - FRAC_PI_2).abs() < 1e-12);
    assert!((a.length() - 10.0 * FRAC_PI_2).abs() < 1e-12);
    assert_eq!(a.radius(), 10.0);
    assert_eq!(a.vertices().len(), 9);
}

#[test]
fn arc_hit_test_follows_the_arc_and_not_the_rest_of_the_circle() {
    let a = quarter_arc();
    let on = p(10.0 * (PI / 4.0).cos(), 10.0 * (PI / 4.0).sin());
    assert!(a.hit_test_point(on, 0.5, false));
    assert!(a.hit_test_point(p(12.0, 0.0), 2.0, false));
    assert!(!a.hit_test_point(p(-10.0, 0.0), 2.0, false));
    assert!(!a.hit_test_point(p(0.0, -10.0), 2.0, false));
}

#[test]
fn arc_nearest_point_clamps_to_the_ends() {
    let a = quarter_arc();
    assert_eq!(a.nearest_point(p(30.0, -1.0)), p(10.0, 0.0));
    assert_eq!(a.nearest_point(p(-1.0, 30.0)), p(0.0, 10.0));
}

#[test]
fn group_hit_test_is_any_child_and_distance_is_the_minimum() {
    let g = Group2d::new(vec![
        Box::new(Edge2d::new(p(0.0, 0.0), p(10.0, 0.0))),
        Box::new(Edge2d::new(p(0.0, 20.0), p(10.0, 20.0))),
    ]);
    assert!(g.hit_test_point(p(5.0, 21.0), 1.0, false));
    assert!(!g.hit_test_point(p(5.0, 10.0), 1.0, false));
    assert_eq!(g.distance_to_point(p(5.0, 18.0), false), 2.0);
    assert_eq!(g.bounds(), BBox::new(0.0, 0.0, 10.0, 20.0));
    assert_eq!(g.vertices().len(), 4);
    assert_eq!(g.nearest_point(p(5.0, 19.0)), p(5.0, 20.0));
}

#[test]
fn group_collects_intersections_from_every_child() {
    let g = Group2d::new(vec![
        Box::new(Edge2d::new(p(0.0, 0.0), p(10.0, 0.0))),
        Box::new(Edge2d::new(p(0.0, 20.0), p(10.0, 20.0))),
    ]);
    assert_eq!(
        g.intersect_line_segment(p(5.0, -5.0), p(5.0, 25.0)).len(),
        2
    );
}

#[test]
fn empty_group_has_zero_bounds_and_no_hit() {
    let g = Group2d::new(vec![]);
    assert_eq!(g.bounds(), BBox::default());
    assert!(!g.hit_test_point(p(0.0, 0.0), 100.0, true));
}

#[test]
fn closed_shape_reports_two_edge_crossings_open_one_reports_per_edge() {
    let sq = square(false);
    let hits = sq.intersect_line_segment(p(-5.0, 5.0), p(15.0, 5.0));
    assert_eq!(hits.len(), 2);
    let e = Ellipse2d::new(100.0, 100.0, false);
    assert_eq!(
        e.intersect_line_segment(p(-5.0, 50.0), p(105.0, 50.0))
            .len(),
        2
    );
    assert_eq!(e.intersect_circle(p(50.0, 50.0), 70.0).len(), 0);
    assert!(e.intersect_circle(p(50.0, 50.0), 20.0).is_empty());
    assert!(!e.intersect_circle(p(100.0, 50.0), 30.0).is_empty());
}

fn brush(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<PVec> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

#[test]
fn marquee_inside_a_filled_shape_selects_it_but_not_a_hollow_one() {
    let inner = brush(4.0, 4.0, 6.0, 6.0);
    assert!(square(true).overlaps_polygon(&inner));
    assert!(!square(false).overlaps_polygon(&inner));
}

#[test]
fn marquee_covering_a_shape_or_touching_a_vertex_selects_it() {
    let cover = brush(-5.0, -5.0, 15.0, 15.0);
    assert!(square(false).overlaps_polygon(&cover));
    let corner = brush(8.0, 8.0, 14.0, 14.0);
    assert!(square(false).overlaps_polygon(&corner));
}

#[test]
fn marquee_crossing_an_edge_without_a_vertex_inside_selects_it() {
    let bar = brush(-5.0, 4.0, 15.0, 6.0);
    assert!(square(false).overlaps_polygon(&bar));
    assert!(!square(false).overlaps_polygon(&brush(20.0, 20.0, 30.0, 30.0)));
}

#[test]
fn open_polyline_marquee_needs_a_vertex_or_a_crossing() {
    let l = Polyline2d::polyline(vec![p(0.0, 0.0), p(20.0, 0.0)]);
    assert!(l.overlaps_polygon(&brush(5.0, -1.0, 8.0, 1.0)));
    assert!(!l.overlaps_polygon(&brush(5.0, 1.0, 8.0, 3.0)));
}

#[test]
fn group_overlap_is_any_child() {
    let g = Group2d::new(vec![
        Box::new(Edge2d::new(p(0.0, 0.0), p(10.0, 0.0))),
        Box::new(Edge2d::new(p(0.0, 20.0), p(10.0, 20.0))),
    ]);
    assert!(g.overlaps_polygon(&brush(4.0, 19.0, 6.0, 21.0)));
    assert!(!g.overlaps_polygon(&brush(4.0, 8.0, 6.0, 12.0)));
}

#[test]
fn transformed_hit_margin_is_in_outer_units() {
    let t = Transformed::new(Arc::new(square(false)), Mat::scale(2.0, 2.0));
    assert!(t.hit_test_point(p(22.0, 10.0), 2.0, false));
    assert!(!t.hit_test_point(p(22.5, 10.0), 2.0, false));
    assert!((t.distance_to_point(p(24.0, 10.0), false) - 4.0).abs() < 1e-9);
    assert_eq!(t.bounds(), BBox::new(0.0, 0.0, 20.0, 20.0));
}

#[test]
fn transformed_maps_intersections_and_nearest_back_out() {
    let t = Transformed::new(Arc::new(square(false)), Mat::translate(100.0, 0.0));
    let hits = t.intersect_line_segment(p(95.0, 5.0), p(115.0, 5.0));
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().any(|h| (h.x - 100.0).abs() < 1e-9));
    assert!(hits.iter().any(|h| (h.x - 110.0).abs() < 1e-9));
    let n = t.nearest_point(p(105.0, -3.0));
    assert!((n.x - 105.0).abs() < 1e-9 && n.y.abs() < 1e-9);
    assert_eq!(t.intersect_circle(p(105.0, 5.0), 2.0).len(), 0);
}

#[test]
fn transformed_of_transformed_composes() {
    let t = Transformed::new(Arc::new(square(false)), Mat::scale(2.0, 2.0))
        .transformed(Mat::translate(5.0, 0.0));
    assert!(t.hit_test_point(p(5.0, 10.0), 0.1, false));
    assert!(t.hit_test_point(p(25.0, 10.0), 0.1, false));
}

#[test]
fn transformed_rotation_keeps_filled_interior_hits() {
    let t = Transformed::new(Arc::new(square(true)), Mat::compose(50.0, 50.0, FRAC_PI_2));
    assert!(t.hit_test_point(p(45.0, 55.0), 0.0, false));
    assert!(!t.hit_test_point(p(55.0, 45.0), 0.0, false));
}
