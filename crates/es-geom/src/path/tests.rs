use super::*;
use crate::vec::PVec;
use kurbo::{ParamCurve, PathEl, Rect, Shape};

fn p(x: f64, y: f64) -> PVec {
    PVec::new(x, y)
}

fn cubic_count(b: &PathBuilder) -> usize {
    b.commands()
        .iter()
        .filter(|c| matches!(c.kind, Kind::Cubic { .. }))
        .count()
}

fn arc_builder(degrees: f64) -> PathBuilder {
    let r = 50.0;
    let t = degrees.to_radians();
    let mut b = PathBuilder::new();
    b.move_to(p(r, 0.0))
        .circular_arc(r, degrees > 180.0, true, p(r * t.cos(), r * t.sin()));
    b
}

fn max_radial_error(b: &PathBuilder, r: f64) -> f64 {
    let mut worst: f64 = 0.0;
    for seg in b.to_bez_path().segments() {
        for i in 0..=20 {
            let q = seg.eval(f64::from(i) / 20.0);
            worst = worst.max((q.x.hypot(q.y) - r).abs());
        }
    }
    worst
}

#[test]
fn arc_uses_one_cubic_per_quarter_turn_up_to_four() {
    for (degrees, segments) in [(60.0, 1), (100.0, 2), (190.0, 3), (280.0, 4), (350.0, 4)] {
        assert_eq!(
            cubic_count(&arc_builder(degrees)),
            segments,
            "{degrees} degrees"
        );
    }
}

#[test]
fn arc_stays_on_the_circle_within_the_cubic_error_bound() {
    for degrees in [60.0, 90.0, 100.0, 180.0, 190.0, 270.0, 280.0, 350.0] {
        let err = max_radial_error(&arc_builder(degrees), 50.0);
        assert!(err < 50.0 * 3e-4, "{degrees} degrees: {err}");
    }
}

#[test]
fn arc_ends_exactly_on_the_requested_point() {
    let b = arc_builder(190.0);
    let last = b.commands().last().map(|c| c.point);
    let t = 190.0f64.to_radians();
    let end = p(50.0 * t.cos(), 50.0 * t.sin());
    assert!(PVec::dist(last.unwrap_or_default(), end) < 1e-9);
}

#[test]
fn quarter_turn_handle_is_the_classic_circle_constant() {
    let mut b = PathBuilder::new();
    b.move_to(p(10.0, 0.0))
        .circular_arc(10.0, false, true, p(0.0, 10.0));
    let Kind::Cubic { cp1, .. } = b.commands()[1].kind else {
        panic!("expected a cubic");
    };
    assert!((cp1.y - 10.0 * 0.552_284_749_830_793_4).abs() < 1e-3);
}

#[test]
fn sweep_flag_picks_the_side_the_arc_bulges() {
    let mut cw = PathBuilder::new();
    cw.move_to(p(0.0, 0.0))
        .circular_arc(10.0, false, true, p(10.0, 10.0));
    let mut ccw = PathBuilder::new();
    ccw.move_to(p(0.0, 0.0))
        .circular_arc(10.0, false, false, p(10.0, 10.0));
    let a = cw.to_bez_path().bounding_box();
    let b = ccw.to_bez_path().bounding_box();
    assert!(a.x1 > 9.9 && a.y0 > -0.1 && a.x0 > -0.1);
    assert!(b.y1 > 9.9 && b.x0 > -0.1 && b.y0 > -0.1);
    assert!((a.y1 - 10.0).abs() < 1e-6 && (b.x1 - 10.0).abs() < 1e-6);
}

#[test]
fn arc_with_zero_radius_is_a_line_and_to_the_same_point_is_nothing() {
    let mut b = PathBuilder::new();
    b.move_to(p(0.0, 0.0))
        .circular_arc(0.0, false, true, p(5.0, 5.0));
    assert_eq!(b.commands()[1].kind, Kind::Line);
    b.circular_arc(5.0, false, true, p(5.0, 5.0));
    assert_eq!(b.commands().len(), 2);
    let mut empty = PathBuilder::new();
    empty.circular_arc(5.0, false, true, p(5.0, 5.0));
    assert!(empty.commands().is_empty());
}

#[test]
fn too_small_a_radius_scales_up_to_a_half_circle() {
    let mut b = PathBuilder::new();
    b.move_to(p(0.0, 0.0))
        .circular_arc(1.0, false, true, p(20.0, 0.0));
    let bb = b.to_bez_path().bounding_box();
    assert!((bb.height() - 10.0).abs() < 0.01);
    assert_eq!(cubic_count(&b), 2);
}

#[test]
fn close_flags_the_last_command_when_it_already_sits_on_the_start() {
    let mut b = PathBuilder::new();
    b.move_to(p(0.0, 0.0))
        .line_to(p(5.0, 0.0))
        .line_to(p(0.0, 0.0))
        .close();
    assert_eq!(b.commands().len(), 3);
    assert!(b.commands()[2].is_close);
}

#[test]
fn close_adds_a_line_back_otherwise_and_is_a_no_op_on_an_empty_path() {
    let mut b = PathBuilder::new();
    b.close();
    assert!(b.commands().is_empty());
    b.move_to(p(1.0, 1.0)).line_to(p(5.0, 0.0)).close();
    let last = b.commands()[2];
    assert!(last.is_close && last.point == p(1.0, 1.0) && last.kind == Kind::Line);
    let els = b.to_bez_path().elements().to_vec();
    assert!(matches!(els.last(), Some(PathEl::ClosePath)));
}

#[test]
fn vertices_sample_curves_and_keep_the_closing_point() {
    let g = geo_path(GeoShape::Ellipse, 100.0, 50.0, 0.0);
    let v = g.vertices(12);
    assert_eq!(v.len(), 1 + 4 * 12);
    assert_eq!(v[0], p(0.0, 25.0));
    let rect = geo_path(GeoShape::Rectangle, 10.0, 20.0, 0.0).vertices(12);
    assert_eq!(rect.len(), 5);
    assert_eq!(rect[4], p(0.0, 0.0));
}

#[test]
fn line_through_points_is_a_polyline() {
    let b = PathBuilder::line_through_points(&[p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)]);
    assert_eq!(b.commands().len(), 3);
    assert!(PathBuilder::line_through_points(&[]).commands().is_empty());
}

#[test]
fn ellipse_path_lies_on_the_ellipse() {
    let path = geo_path(GeoShape::Ellipse, 200.0, 80.0, 0.0).to_bez_path();
    for seg in path.segments() {
        for i in 0..=20 {
            let q = seg.eval(f64::from(i) / 20.0);
            let k = ((q.x - 100.0) / 100.0).powi(2) + ((q.y - 40.0) / 40.0).powi(2);
            assert!((k.sqrt() - 1.0).abs() < 3e-4);
        }
    }
    let bb = path.bounding_box();
    assert!((bb.width() - 200.0).abs() < 1e-6 && (bb.height() - 80.0).abs() < 1e-6);
}

#[test]
fn rectangle_radius_at_or_below_half_a_pixel_is_a_plain_box() {
    assert_eq!(
        cubic_count(&geo_path(GeoShape::Rectangle, 50.0, 30.0, 0.5)),
        0
    );
    assert_eq!(
        cubic_count(&geo_path(GeoShape::Rectangle, 50.0, 30.0, 0.0)),
        0
    );
    assert_eq!(
        cubic_count(&geo_path(GeoShape::Rectangle, 50.0, 30.0, 6.0)),
        4
    );
}

#[test]
fn rounded_rectangle_clamps_radius_to_half_the_shorter_side_and_keeps_its_box() {
    let path = geo_path(GeoShape::Rectangle, 50.0, 30.0, 999.0).to_bez_path();
    let bb = path.bounding_box();
    assert!((bb.width() - 50.0).abs() < 1e-6 && (bb.height() - 30.0).abs() < 1e-6);
    let b = geo_path(GeoShape::Rectangle, 50.0, 30.0, 999.0);
    assert_eq!(b.commands()[0].point, p(15.0, 0.0));
}

#[test]
fn geo_paths_floor_the_size_at_one() {
    let bb = geo_path(GeoShape::Rectangle, 0.0, -5.0, 0.0)
        .to_bez_path()
        .bounding_box();
    assert_eq!((bb.width(), bb.height()), (1.0, 1.0));
}

#[test]
fn per_corner_radii_path_keeps_the_rect_and_rounds_only_requested_corners() {
    let radii = PerCornerRadii {
        top_left: 10.0,
        top_right: 0.0,
        bottom_left: 0.0,
        bottom_right: 20.0,
    };
    let rect = Rect::new(5.0, 5.0, 105.0, 65.0);
    let path = radii.path_in(rect);
    assert_eq!(cubic_count(&path), 2);
    let bb = path.to_bez_path().bounding_box();
    assert!((bb.x0 - 5.0).abs() < 1e-9 && (bb.x1 - 105.0).abs() < 1e-9);
    assert!((bb.y0 - 5.0).abs() < 1e-9 && (bb.y1 - 65.0).abs() < 1e-9);
    assert!(!radii.is_uniform() && PerCornerRadii::uniform(4.0).is_uniform());
}

#[test]
fn per_corner_radii_clamp_to_half_the_shorter_side() {
    let path = PerCornerRadii::uniform(500.0).path_in(Rect::new(0.0, 0.0, 100.0, 40.0));
    let Kind::Line = path.commands()[1].kind else {
        panic!("expected a line");
    };
    assert_eq!(path.commands()[0].point, p(20.0, 0.0));
    assert_eq!(path.commands()[1].point, p(80.0, 0.0));
}

#[test]
fn outset_grows_round_corners_only() {
    let o = PerCornerRadii {
        top_left: 8.0,
        top_right: 0.0,
        bottom_left: 4.0,
        bottom_right: 0.0,
    }
    .outset_by(3.0);
    assert_eq!(
        (o.top_left, o.top_right, o.bottom_left, o.bottom_right),
        (11.0, 0.0, 7.0, 0.0)
    );
}

fn dash(total: f64, sw: f64, style: DashStyle) -> DashProps {
    perfect_dash_props(total, sw, style, &DashOptions::default())
}

#[test]
fn solid_and_draw_styles_have_no_dashes() {
    assert_eq!(dash(100.0, 4.0, DashStyle::Solid), DashProps::default());
    assert_eq!(dash(100.0, 4.0, DashStyle::Draw), DashProps::default());
}

#[test]
fn dashed_run_fits_a_whole_number_of_dashes_with_both_ends_on_a_dash() {
    let props = dash(200.0, 8.0, DashStyle::Dashed);
    assert_eq!(props.dashes.len(), 2);
    assert!((props.dashes[0] - 18.0).abs() < 1e-12);
    assert!((props.dashes[1] - 21.6).abs() < 1e-12);
    assert!((props.phase + 8.0).abs() < 1e-12);
    let run = 6.0 * props.dashes[0] + 5.0 * props.dashes[1];
    assert!((run - (200.0 + 16.0)).abs() < 1e-9);
}

#[test]
fn dashed_dash_count_scales_with_length() {
    let count = |total: f64| {
        let p = dash(total, 4.0, DashStyle::Dashed);
        ((total + 8.0 + p.dashes[1]) / (p.dashes[0] + p.dashes[1])).round() as i64
    };
    assert!(count(400.0) > count(200.0));
}

#[test]
fn short_dashed_run_becomes_one_solid_dash_or_thirds() {
    let one = dash(20.0, 8.0, DashStyle::Dashed);
    assert_eq!(one.dashes, vec![25.0, 0.0]);
    let thirds = dash(60.0, 8.0, DashStyle::Dashed);
    assert_eq!(thirds.dashes, vec![25.0, 25.0]);
}

#[test]
fn dotted_dashes_are_a_hundredth_of_the_stroke_width_and_tile_the_run() {
    let props = dash(100.0, 8.0, DashStyle::Dotted);
    let (d, g) = (props.dashes[0], props.dashes[1]);
    assert!((d - 0.08).abs() < 0.01);
    let n = ((100.08 + g) / (d + g)).round();
    assert!((n * d + (n - 1.0) * g - 100.08).abs() < 1e-9);
    assert!(g > d);
}

#[test]
fn closed_runs_offset_by_half_a_dash_and_spread_gaps_over_every_dash() {
    let opts = DashOptions {
        closed: true,
        ..DashOptions::default()
    };
    let props = perfect_dash_props(240.0, 8.0, DashStyle::Dashed, &opts);
    let (d, g) = (props.dashes[0], props.dashes[1]);
    assert!((props.phase + d / 2.0).abs() < 1e-12);
    let count = 240.0 / (2.0 * d);
    assert!((count - count.round()).abs() < 1e-9);
    assert!((count * (d + g) - 240.0).abs() < 1e-9);
}

#[test]
fn terminals_change_the_fitted_length() {
    let skip = DashOptions {
        start: DashTerminal::Skip,
        end: DashTerminal::Skip,
        ..DashOptions::default()
    };
    let none = DashOptions {
        start: DashTerminal::None,
        end: DashTerminal::None,
        ..DashOptions::default()
    };
    let a = perfect_dash_props(200.0, 8.0, DashStyle::Dashed, &skip);
    let b = perfect_dash_props(200.0, 8.0, DashStyle::Dashed, &none);
    assert!((a.phase - 16.0).abs() < 1e-12);
    assert_eq!(b.phase, 0.0);
    assert!((a.dashes[0] * 5.0 + a.dashes[1] * 4.0 - (200.0 - 32.0)).abs() < 1e-9);
}

#[test]
fn snap_rounds_the_dash_count_down() {
    let opts = DashOptions {
        snap: 4,
        ..DashOptions::default()
    };
    let props = perfect_dash_props(200.0, 8.0, DashStyle::Dashed, &opts);
    assert!((props.dashes[0] - 216.0 / 4.0 / 2.0).abs() < 1e-12);
}

#[test]
fn degenerate_inputs_give_no_dashes() {
    assert_eq!(dash(100.0, 0.0, DashStyle::Dashed), DashProps::default());
    assert_eq!(dash(0.0, 4.0, DashStyle::Dotted), DashProps::default());
    assert_eq!(dash(f64::NAN, 4.0, DashStyle::Dashed), DashProps::default());
}

#[test]
fn each_side_of_a_rectangle_gets_a_pattern_fitted_to_its_own_length() {
    let segs =
        geo_path(GeoShape::Rectangle, 200.0, 100.0, 0.0).dashed_segments(8.0, DashStyle::Dashed);
    assert_eq!(segs.len(), 4);
    assert_eq!(segs[0].dashes, segs[2].dashes);
    assert_eq!(segs[1].dashes, segs[3].dashes);
    assert_ne!(segs[0].dashes, segs[1].dashes);
    for (seg, total) in segs.iter().zip([200.0, 100.0, 200.0, 100.0]) {
        let d = seg.dashes[0];
        let g = seg.dashes[1];
        let count = ((total + 16.0 + g) / (d + g)).round();
        assert!(
            (count * d + (count - 1.0) * g - (total + 16.0)).abs() < 1e-9,
            "side {total}"
        );
        assert!((seg.phase + 8.0).abs() < 1e-12);
    }
}

#[test]
fn curves_are_dashed_by_their_arc_length_and_solid_has_empty_patterns() {
    let segs =
        geo_path(GeoShape::Ellipse, 200.0, 100.0, 0.0).dashed_segments(4.0, DashStyle::Dashed);
    assert_eq!(segs.len(), 4);
    assert!(segs.iter().all(|s| s.dashes.len() == 2));
    let solid =
        geo_path(GeoShape::Ellipse, 200.0, 100.0, 0.0).dashed_segments(4.0, DashStyle::Solid);
    assert!(solid.iter().all(|s| s.dashes.is_empty() && s.phase == 0.0));
}

#[test]
fn zero_length_commands_are_not_dashed() {
    let mut b = PathBuilder::new();
    b.move_to(p(0.0, 0.0))
        .line_to(p(0.0, 0.0))
        .line_to(p(10.0, 0.0));
    assert_eq!(b.dashed_segments(2.0, DashStyle::Dashed).len(), 1);
}

fn points_of(path: &kurbo::BezPath) -> Vec<kurbo::Point> {
    path.elements()
        .iter()
        .filter_map(|e| match e {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(*p),
            _ => None,
        })
        .collect()
}

#[test]
fn draw_path_is_reproducible_from_its_seed_and_differs_between_seeds() {
    let rect = geo_path(GeoShape::Rectangle, 120.0, 80.0, 0.0);
    let a = rect.draw_path(&DrawOptions::new(6.0, "shape-1"));
    let b = rect.draw_path(&DrawOptions::new(6.0, "shape-1"));
    let c = rect.draw_path(&DrawOptions::new(6.0, "shape-2"));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn draw_path_makes_one_subpath_per_pass() {
    let rect = geo_path(GeoShape::Rectangle, 120.0, 80.0, 0.0);
    let moves = |passes| {
        let mut o = DrawOptions::new(6.0, "");
        o.passes = passes;
        rect.draw_path(&o)
            .elements()
            .iter()
            .filter(|e| matches!(e, PathEl::MoveTo(_)))
            .count()
    };
    assert_eq!(moves(1), 1);
    assert_eq!(moves(2), 2);
    assert_eq!(moves(3), 3);
}

#[test]
fn draw_path_without_jitter_or_roundness_traces_the_shape() {
    let rect = geo_path(GeoShape::Rectangle, 120.0, 80.0, 0.0);
    let mut o = DrawOptions::new(6.0, "x");
    o.offset = Some(0.0);
    o.roundness = Some(0.0);
    o.passes = 1;
    let pts = points_of(&rect.draw_path(&o));
    let want = [
        (0.0, 0.0),
        (120.0, 0.0),
        (120.0, 80.0),
        (0.0, 80.0),
        (0.0, 0.0),
    ];
    assert_eq!(pts.len(), want.len());
    for (got, w) in pts.iter().zip(want) {
        assert!((got.x - w.0).abs() < 1e-9 && (got.y - w.1).abs() < 1e-9);
    }
}

#[test]
fn draw_path_jitter_stays_within_a_third_of_the_stroke_width_and_closes_on_its_start() {
    let rect = geo_path(GeoShape::Rectangle, 300.0, 200.0, 0.0);
    let mut o = DrawOptions::new(9.0, "jitter");
    o.roundness = Some(0.0);
    o.passes = 1;
    let pts = points_of(&rect.draw_path(&o));
    let want = [
        (0.0, 0.0),
        (300.0, 0.0),
        (300.0, 200.0),
        (0.0, 200.0),
        (0.0, 0.0),
    ];
    for (got, w) in pts.iter().zip(want) {
        assert!((got.x - w.0).hypot(got.y - w.1) <= 3.0 + 1e-9);
    }
    assert_eq!(pts[0], pts[4]);
    assert!(
        pts.iter()
            .zip(want)
            .any(|(g, w)| (g.x - w.0).hypot(g.y - w.1) > 1e-6)
    );
}

#[test]
fn draw_path_rounds_right_angles_with_a_quadratic_and_leaves_a_fold_sharp() {
    let quads = |points: &[PVec]| {
        let mut o = DrawOptions::new(6.0, "q");
        o.offset = Some(0.0);
        o.passes = 1;
        PathBuilder::line_through_points(points)
            .draw_path(&o)
            .elements()
            .iter()
            .filter(|e| matches!(e, PathEl::QuadTo(..)))
            .count()
    };
    assert_eq!(quads(&[p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)]), 1);
    assert_eq!(quads(&[p(0.0, 0.0), p(100.0, 0.0), p(0.0, 0.0)]), 0);
}

#[test]
fn per_command_offset_overrides_the_default() {
    let mut b = PathBuilder::new();
    b.move_to_with(
        p(0.0, 0.0),
        CommandOpts {
            offset: Some(0.0),
            roundness: None,
        },
    )
    .line_to_with(
        p(100.0, 0.0),
        CommandOpts {
            offset: Some(0.0),
            roundness: None,
        },
    );
    let mut o = DrawOptions::new(9.0, "");
    o.passes = 1;
    let pts = points_of(&b.draw_path(&o));
    assert_eq!(
        pts,
        vec![kurbo::Point::new(0.0, 0.0), kurbo::Point::new(100.0, 0.0)]
    );
}

#[test]
fn draw_path_gives_a_straight_join_a_quadratic_whose_control_stays_on_the_line() {
    let mut o = DrawOptions::new(6.0, "s");
    o.offset = Some(0.0);
    o.passes = 1;
    let path = PathBuilder::line_through_points(&[p(0.0, 0.0), p(100.0, 0.0), p(200.0, 0.0)])
        .draw_path(&o);
    let quads: Vec<_> = path
        .elements()
        .iter()
        .filter_map(|e| match e {
            PathEl::QuadTo(c, _) => Some(*c),
            _ => None,
        })
        .collect();
    assert_eq!(quads.len(), 1);
    assert!((quads[0].x - 100.0).abs() < 1e-9 && quads[0].y.abs() < 1e-9);
}
