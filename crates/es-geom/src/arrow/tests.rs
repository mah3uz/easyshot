use super::*;
use crate::bbox::BBox;
use crate::geometry2d::{Ellipse2d, Geometry2d, Polyline2d};
use crate::mat::Mat;
use crate::path::DashStyle;
use crate::vec::PVec;
use kurbo::Shape;
use std::f64::consts::PI;

fn p(x: f64, y: f64) -> PVec {
    PVec::new(x, y)
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn input<'a>(start: PVec, end: PVec, bend: f64) -> ArrowInput<'a> {
    ArrowInput {
        transform: Mat::IDENTITY,
        start,
        end,
        bend,
        head_start: Arrowhead::None,
        head_end: Arrowhead::None,
        stroke_width: 2.0,
        start_bound: None,
        end_bound: None,
    }
}

fn bound<'a>(
    id: u64,
    geometry: &'a dyn Geometry2d,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke_width: f64,
) -> Bound<'a> {
    Bound {
        id,
        geometry,
        transform: Mat::translate(x, y),
        page_bounds: BBox::new(x, y, w, h),
        stroke_width,
        normalized_anchor: p(0.5, 0.5),
        precise: false,
        exact: false,
    }
}

fn rect(w: f64, h: f64) -> Polyline2d {
    Polyline2d::rectangle(0.0, 0.0, w, h, true)
}

fn straight_end(x: f64, head: Arrowhead, sw: f64) -> ArrowInfo {
    let mut i = input(p(0.0, 0.0), p(x, 0.0), 0.0);
    i.head_end = head;
    i.stroke_width = sw;
    resolve(&i)
}

#[test]
fn bends_below_the_minimum_snap_straight_and_the_threshold_itself_curves() {
    assert!(is_straight(7.99));
    assert!(is_straight(-7.99));
    assert!(!is_straight(8.0));
    assert!(!is_straight(-8.0));
    assert_eq!(snap_bend(7.0), 0.0);
    assert_eq!(snap_bend(-7.0), 0.0);
    assert_eq!(snap_bend(8.0), 8.0);
    assert_eq!(snap_bend(-30.0), -30.0);
}

#[test]
fn dragging_the_middle_handle_reproduces_the_bend_that_placed_it() {
    for bend in [-60.0, -9.0, 0.0, 12.0, 80.0] {
        let info = resolve(&input(p(10.0, 20.0), p(140.0, 90.0), bend));
        let back = bend_for_middle(p(10.0, 20.0), p(140.0, 90.0), info.middle);
        if is_straight(bend) {
            assert!(
                near(back, 0.0, 1e-9),
                "straight arrows keep the chord midpoint"
            );
        } else {
            assert!(near(back, bend, 1e-9), "bend {bend} came back as {back}");
        }
    }
}

#[test]
fn zero_bend_is_a_straight_body_and_a_real_bend_is_an_arc_with_a_handle_arc() {
    let straight = resolve(&input(p(0.0, 0.0), p(100.0, 0.0), 0.0));
    assert!(straight.is_straight());
    assert!(straight.handle_arc.is_none());
    assert_eq!(straight.middle, p(50.0, 0.0));

    let nearly = resolve(&input(p(0.0, 0.0), p(100.0, 0.0), 7.9));
    assert!(nearly.is_straight());

    let curved = resolve(&input(p(0.0, 0.0), p(100.0, 0.0), 8.0));
    assert!(!curved.is_straight());
    assert!(curved.handle_arc.is_some());
    assert!(near(curved.middle.x, 50.0, 1e-9) && near(curved.middle.y, 8.0, 1e-9));
}

#[test]
fn unbound_arrow_body_runs_between_its_own_terminals() {
    let info = resolve(&input(p(5.0, 5.0), p(105.0, 5.0), 0.0));
    assert_eq!(info.start.point, p(5.0, 5.0));
    assert_eq!(info.end.point, p(105.0, 5.0));
    assert!(info.is_valid);
}

#[test]
fn zero_length_arrows_are_invalid_but_never_produce_nan() {
    for bend in [0.0, 40.0] {
        let mut i = input(p(30.0, 30.0), p(30.0, 30.0), bend);
        i.head_start = Arrowhead::Triangle;
        i.head_end = Arrowhead::Dot;
        let info = resolve(&i);
        assert!(!info.is_valid);
        assert!(info.start.point.is_finite() && info.end.point.is_finite());
        for side in [Terminal::Start, Terminal::End] {
            let hp = info.head_points(side);
            assert!(hp.int.is_finite());
            assert!(info.head_path(side).is_some());
        }
        assert!(
            info.body_builder()
                .to_bez_path()
                .bounding_box()
                .area()
                .is_finite()
        );
        let g = info.geometry();
        assert!(g.hit_test_point(p(30.0, 30.0), 3.0, false));
    }
}

#[test]
fn end_bound_to_a_rectangle_stops_at_its_edge_then_backs_off_for_the_head() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(0.0, 50.0), p(150.0, 50.0), 0.0);
    i.end_bound = Some(bound(1, &r, 100.0, 0.0, 100.0, 100.0, 2.0));
    let headless = resolve(&i);
    assert!(near(headless.end.point.x, 100.0, 1e-6));
    assert!(near(headless.end.handle.x, 150.0, 1e-6));

    i.head_end = Arrowhead::Arrow;
    let headed = resolve(&i);
    let offset = 10.0 + 2.0 / 2.0 + 2.0 / 2.0;
    assert!(near(headed.end.point.x, 100.0 - offset, 1e-6));
}

#[test]
fn exact_binding_stops_at_the_terminal_without_clipping_or_offset() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(0.0, 50.0), p(150.0, 50.0), 0.0);
    i.head_end = Arrowhead::Arrow;
    let mut b = bound(1, &r, 100.0, 0.0, 100.0, 100.0, 2.0);
    b.exact = true;
    i.end_bound = Some(b);
    let info = resolve(&i);
    assert!(near(info.end.point.x, 150.0, 1e-6));
}

#[test]
fn end_bound_to_an_ellipse_stops_on_the_ellipse_outline() {
    let e = Ellipse2d::new(100.0, 100.0, true);
    let mut i = input(p(0.0, 50.0), p(150.0, 50.0), 0.0);
    i.end_bound = Some(bound(1, &e, 100.0, 0.0, 100.0, 100.0, 2.0));
    let info = resolve(&i);
    assert!(
        near(info.end.point.x, 100.0, 1.5),
        "got {}",
        info.end.point.x
    );
    assert!(near(info.end.point.y, 50.0, 1e-6));
}

#[test]
fn start_bound_clips_the_start_the_same_way() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(50.0, 50.0), p(300.0, 50.0), 0.0);
    i.start_bound = Some(bound(1, &r, 0.0, 0.0, 100.0, 100.0, 2.0));
    let info = resolve(&i);
    assert!(near(info.start.point.x, 100.0, 1e-6));
    assert!(near(info.start.handle.x, 50.0, 1e-6));
}

#[test]
fn imprecise_binding_aims_at_the_center_and_precise_at_the_clamped_anchor() {
    let r = rect(100.0, 60.0);
    let mut i = input(p(0.0, 0.0), p(300.0, 300.0), 0.0);
    let mut b = bound(1, &r, 200.0, 100.0, 100.0, 60.0, 2.0);
    b.normalized_anchor = p(0.0, 2.0);
    i.end_bound = Some(b);
    let imprecise = resolve(&i);
    assert_eq!(imprecise.end.handle, p(250.0, 130.0));

    let mut b = bound(1, &r, 200.0, 100.0, 100.0, 60.0, 2.0);
    b.normalized_anchor = p(0.0, 2.0);
    b.precise = true;
    i.end_bound = Some(b);
    let precise = resolve(&i);
    assert!(near(precise.end.handle.x, 200.0 + 100.0 * 1e-3, 1e-9));
    assert!(near(
        precise.end.handle.y,
        100.0 + 60.0 * (1.0 - 1e-3),
        1e-9
    ));
}

#[test]
fn two_ends_on_one_shape_use_their_precise_anchors() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(0.0, 0.0), p(0.0, 0.0), 0.0);
    let mut s = bound(1, &r, 0.0, 0.0, 100.0, 100.0, 2.0);
    s.normalized_anchor = p(0.1, 0.5);
    let mut e = bound(1, &r, 0.0, 0.0, 100.0, 100.0, 2.0);
    e.normalized_anchor = p(0.9, 0.5);
    i.start_bound = Some(s);
    i.end_bound = Some(e);
    let info = resolve(&i);
    assert!(near(info.start.handle.x, 10.0, 1e-9));
    assert!(near(info.end.handle.x, 90.0, 1e-9));
}

#[test]
fn a_gap_too_tight_for_both_heads_extends_the_body_behind_the_shape_edges() {
    let a = rect(100.0, 100.0);
    let b = rect(100.0, 100.0);
    let mut i = input(p(50.0, 50.0), p(170.0, 50.0), 0.0);
    i.head_start = Arrowhead::Arrow;
    i.head_end = Arrowhead::Arrow;
    i.start_bound = Some(bound(1, &a, 0.0, 0.0, 100.0, 100.0, 2.0));
    i.end_bound = Some(bound(2, &b, 120.0, 0.0, 100.0, 100.0, 2.0));
    let info = resolve(&i);
    let offset = 10.0 + 1.0 + 1.0;
    assert!(near(info.start.point.x, 100.0 - 1.5 * offset, 1e-6));
    assert!(near(info.end.point.x, 120.0 + 1.5 * offset, 1e-6));
}

#[test]
fn bound_arrows_report_resolved_terminals_to_store_so_export_follows_the_shapes() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(-1000.0, -1000.0), p(-2000.0, -2000.0), 0.0);
    i.transform = Mat::translate(10.0, 10.0);
    i.start_bound = Some(bound(1, &r, 200.0, 0.0, 100.0, 100.0, 2.0));
    i.end_bound = Some(bound(2, &r, 400.0, 100.0, 100.0, 100.0, 2.0));
    let (s, e) = resolve(&i).synced_terminals();
    assert_eq!(s, p(250.0 - 10.0, 50.0 - 10.0));
    assert_eq!(e, p(450.0 - 10.0, 150.0 - 10.0));
}

#[test]
fn partly_bound_arrow_syncs_only_the_bound_end() {
    let r = rect(100.0, 100.0);
    let mut i = input(p(7.0, 8.0), p(-2000.0, -2000.0), 0.0);
    i.end_bound = Some(bound(2, &r, 400.0, 100.0, 100.0, 100.0, 2.0));
    let (s, e) = resolve(&i).synced_terminals();
    assert_eq!(s, p(7.0, 8.0));
    assert_eq!(e, p(450.0, 150.0));
}

fn on_boundary(g: &dyn Geometry2d, pt: PVec) -> bool {
    PVec::dist(pt, g.nearest_point(pt)) < 1e-6
}

#[test]
fn curved_arrow_clips_to_the_rectangle_along_its_own_circle() {
    let r = rect(80.0, 80.0);
    let mut i = input(p(0.0, 100.0), p(260.0, 100.0), -60.0);
    i.end_bound = Some(bound(1, &r, 200.0, 60.0, 80.0, 80.0, 2.0));
    let info = resolve(&i);
    let arc = info.handle_arc.expect("curved");
    let end = info.end.point;
    assert!(near(PVec::dist(end, arc.center), arc.radius, 1e-6));
    let local = p(end.x - 200.0, end.y - 60.0);
    assert!(
        on_boundary(&r, local),
        "end {end:?} is not on the rectangle"
    );
    assert!(info.is_valid);
}

#[test]
fn curved_arrow_with_head_backs_off_the_bound_shape_along_the_arc() {
    let r = rect(80.0, 80.0);
    let mut i = input(p(0.0, 100.0), p(260.0, 100.0), -60.0);
    i.head_end = Arrowhead::Triangle;
    i.end_bound = Some(bound(1, &r, 200.0, 60.0, 80.0, 80.0, 2.0));
    let backed = resolve(&i);
    i.head_end = Arrowhead::None;
    let flush = resolve(&i);
    let arc = backed.handle_arc.expect("curved");
    assert!(near(
        PVec::dist(backed.end.point, arc.center),
        arc.radius,
        1e-6
    ));
    let gap = PVec::dist(backed.end.point, flush.end.point);
    assert!(gap > 10.0 && gap < 13.5, "backed off by {gap}");
}

#[test]
fn curved_arrow_bound_at_both_ends_keeps_the_middle_handle_between_the_bodies_ends() {
    let a = rect(60.0, 60.0);
    let b = rect(60.0, 60.0);
    let mut i = input(p(0.0, 0.0), p(0.0, 0.0), 50.0);
    i.start_bound = Some(bound(1, &a, 0.0, 0.0, 60.0, 60.0, 2.0));
    i.end_bound = Some(bound(2, &b, 240.0, 0.0, 60.0, 60.0, 2.0));
    let info = resolve(&i);
    assert!(info.is_valid);
    assert!(info.start.point.x > 55.0 && info.start.point.x < 130.0);
    assert!(info.end.point.x < 245.0 && info.end.point.x > 170.0);
    assert!(info.middle.x > info.start.point.x && info.middle.x < info.end.point.x);
}

fn head_len(arrow_len: f64, sw: f64) -> f64 {
    (arrow_len / 5.0).clamp(sw, sw * 3.0)
}

#[test]
fn head_length_is_a_fifth_of_the_arrow_clamped_to_one_to_three_stroke_widths() {
    assert!(near(head_len(200.0, 2.0), 6.0, 1e-12));
    assert!(near(head_len(200.0, 20.0), 40.0, 1e-12));
    assert!(near(head_len(10.0, 4.0), 4.0, 1e-12));

    for (arrow_len, sw) in [(200.0, 2.0), (200.0, 20.0), (10.0, 4.0), (30.0, 1.0)] {
        let info = straight_end(arrow_len, Arrowhead::Triangle, sw);
        let hp = info.head_points(Terminal::End);
        assert!(near(
            PVec::dist(hp.point, hp.int),
            head_len(arrow_len, sw),
            1e-9
        ));
        assert!(hp.int.x < hp.point.x, "base lies behind the tip");
    }
}

#[test]
fn open_arrow_head_is_stroked_only_and_spans_the_wing_angle() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Arrow, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(!h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.x1, 200.0, 1e-9));
        assert!(near(b.width(), l * (PI / 6.0).cos(), 1e-9));
        assert!(near(b.height(), l, 1e-9));
    }
}

#[test]
fn triangle_head_is_filled_and_closed() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Triangle, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.width(), l * (PI / 6.0).cos(), 1e-9));
        assert!(near(b.height(), l, 1e-9));
        assert!(
            h.path
                .elements()
                .iter()
                .any(|e| matches!(e, kurbo::PathEl::ClosePath))
        );
    }
}

#[test]
fn inverted_head_puts_its_flat_side_on_the_tip_and_its_point_on_the_body() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Inverted, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.x1, 200.0, 1e-9) && near(b.x0, 200.0 - l, 1e-9));
        assert!(near(b.height(), l, 1e-9));
    }
}

#[test]
fn dot_head_is_a_filled_circle_of_forty_five_percent_of_the_head_length() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Dot, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(h.filled);
        let r = 0.45 * head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(
            near(b.width(), 2.0 * r, 0.05 * r),
            "width {} vs {}",
            b.width(),
            2.0 * r
        );
        assert!(near(b.height(), 2.0 * r, 0.05 * r));
        assert!(near(b.x1, 200.0, 0.05 * r));
    }
}

#[test]
fn diamond_head_reaches_past_the_base_point_by_the_reflection_of_its_wings() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Diamond, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.width(), 1.5 * l * (PI / 4.0).cos(), 1e-9));
        assert!(near(b.height(), 1.5 * l * (PI / 4.0).sin(), 1e-9));
    }
}

#[test]
fn square_head_is_eighty_five_percent_long_and_as_wide_as_it_is_long() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Square, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.width(), 0.85 * l, 1e-9));
        assert!(near(b.height(), 0.85 * l, 1e-9));
        assert!(near(b.x1, 200.0, 1e-9));
    }
}

#[test]
fn bar_head_is_a_stroked_line_across_the_tip() {
    for sw in [2.0, 20.0] {
        let info = straight_end(200.0, Arrowhead::Bar, sw);
        let h = info.head_path(Terminal::End).expect("head");
        assert!(!h.filled);
        let l = head_len(200.0, sw);
        let b = h.path.bounding_box();
        assert!(near(b.width(), 0.0, 1e-9));
        assert!(near(b.height(), l, 1e-9));
        assert!(near(b.x0, 200.0, 1e-9));
    }
}

#[test]
fn no_head_means_no_head_path_and_start_heads_sit_at_the_start() {
    let info = straight_end(200.0, Arrowhead::None, 2.0);
    assert!(info.head_path(Terminal::End).is_none());
    assert!(info.head_path(Terminal::Start).is_none());

    let mut i = input(p(0.0, 0.0), p(200.0, 0.0), 0.0);
    i.head_start = Arrowhead::Triangle;
    let info = resolve(&i);
    let b = info
        .head_path(Terminal::Start)
        .expect("head")
        .path
        .bounding_box();
    assert!(near(b.x0, 0.0, 1e-9) && b.x1 > 0.0);
}

#[test]
fn heads_on_a_curved_arrow_sit_on_the_arc_behind_the_tip() {
    for bend in [40.0, -40.0] {
        let mut i = input(p(0.0, 0.0), p(200.0, 0.0), bend);
        i.head_start = Arrowhead::Triangle;
        i.head_end = Arrowhead::Triangle;
        let info = resolve(&i);
        let arc = info.handle_arc.expect("curved");
        for side in [Terminal::Start, Terminal::End] {
            let hp = info.head_points(side);
            assert!(near(PVec::dist(hp.int, arc.center), arc.radius, 1e-6));
            let ArrowBody::Arc(body) = info.body else {
                panic!("arc body")
            };
            let len = head_len(body.length.abs(), 2.0);
            assert!(near(PVec::dist(hp.int, hp.point), len, 1e-6));
            let toward_middle = PVec::dist(hp.int, info.middle) < PVec::dist(hp.point, info.middle);
            assert!(
                toward_middle,
                "bend {bend} {side:?}: base must lie toward the body"
            );
        }
    }
}

#[test]
fn dashed_styles_split_the_body_and_solid_leaves_it_whole() {
    let info = resolve(&input(p(0.0, 0.0), p(200.0, 0.0), 0.0));
    let solid = info
        .body_builder()
        .dashed_segments(info.stroke_width, DashStyle::Solid);
    assert_eq!(solid.len(), 1);
    assert!(solid[0].dashes.is_empty());
    let dashed = info
        .body_builder()
        .dashed_segments(info.stroke_width, DashStyle::Dashed);
    assert_eq!(dashed.len(), 1);
    assert!(!dashed[0].dashes.is_empty());

    let curved = resolve(&input(p(0.0, 0.0), p(200.0, 0.0), 40.0));
    assert!(
        !curved
            .body_builder()
            .dashed_segments(curved.stroke_width, DashStyle::Dotted)
            .is_empty()
    );
}

#[test]
fn hit_geometry_covers_the_body_and_the_head_but_not_the_empty_space_beside_them() {
    let mut i = input(p(0.0, 0.0), p(100.0, 0.0), 0.0);
    i.head_end = Arrowhead::Triangle;
    let g = resolve(&i).geometry();
    assert!(g.hit_test_point(p(50.0, 2.0), 3.0, false));
    assert!(g.hit_test_point(p(98.0, 0.5), 1.0, false));
    assert!(!g.hit_test_point(p(50.0, 12.0), 3.0, false));
}

#[test]
fn hit_geometry_of_a_curved_arrow_follows_the_arc_not_the_chord() {
    let info = resolve(&input(p(0.0, 0.0), p(100.0, 0.0), 40.0));
    let g = info.geometry();
    assert!(g.hit_test_point(info.middle, 2.0, false));
    assert!(!g.hit_test_point(p(50.0, 0.0), 3.0, false));
}
