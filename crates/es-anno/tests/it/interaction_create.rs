use crate::common;

use common::*;
use es_anno::{CursorKind, Editor, PointerInfo};
use es_geom::{PVec, Point};
use es_model::{AnnoShape, Arrowhead, FillStyle, GeoKind, RedactionKind, ShapeKind, Tool};

fn armed(tool: Tool) -> Editor {
    let mut ed = editor(vec![], vec![]);
    ed.select_tool(tool);
    ed
}

fn only_shape(ed: &Editor) -> AnnoShape {
    assert_eq!(ed.doc().shapes().len(), 1, "exactly one shape");
    ed.doc().shapes()[0].clone()
}

fn frame(s: &AnnoShape) -> (f64, f64, f64, f64) {
    match &s.kind {
        ShapeKind::Geo(p) => (s.x, s.y, p.w, p.h),
        ShapeKind::Redaction(p) => (s.x, s.y, p.w, p.h),
        ShapeKind::Highlight(p) => (s.x, s.y, p.w, p.h),
        other => panic!("not a box shape: {other:?}"),
    }
}

fn shift(p: PointerInfo) -> PointerInfo {
    PointerInfo { shift: true, ..p }
}

fn alt(p: PointerInfo) -> PointerInfo {
    PointerInfo { alt: true, ..p }
}

fn click(ed: &mut Editor, p: PointerInfo) {
    ed.pointer_down(p);
    ed.pointer_up(p);
}

#[test]
fn a_rectangle_drag_spans_press_to_release_in_the_current_style_and_is_one_undo_step() {
    let mut ed = armed(Tool::Rectangle);
    let before = ed.doc().snapshot();
    ed.pointer_down(at(100.0, 100.0));
    assert_eq!(
        ed.cursor_for(Point::new(100.0, 100.0)),
        CursorKind::DrawRectangle
    );
    for x in [120.0, 180.0, 250.0] {
        ed.pointer_move(at(x, 160.0));
    }
    ed.pointer_up(at(250.0, 160.0));
    let s = only_shape(&ed);
    assert_eq!(frame(&s), (100.0, 100.0, 150.0, 60.0));
    let ShapeKind::Geo(p) = &s.kind else {
        unreachable!()
    };
    assert_eq!((p.geo, p.fill), (GeoKind::Rectangle, FillStyle::None));
    assert_eq!(p.swatch, ed.current_style().swatch);
    assert_eq!(
        p.stroke_width,
        ed.viewport()
            .page_stroke_width(ed.current_style().stroke_width)
    );
    assert_eq!(s.rotation, 0.0);
    assert!(ed.selected_ids().contains(&s.id) && ed.selected_ids().len() == 1);
    assert!(ed.is_idle());
    one_undo_entry(&mut ed, &before);
}

#[test]
fn dragging_up_and_left_keeps_the_box_between_press_and_pointer() {
    let mut ed = armed(Tool::Ellipse);
    drag(&mut ed, at(300.0, 300.0), at(200.0, 250.0));
    let s = only_shape(&ed);
    assert_eq!(frame(&s), (200.0, 250.0, 100.0, 50.0));
    assert!(matches!(&s.kind, ShapeKind::Geo(p) if p.geo == GeoKind::Ellipse));
}

#[test]
fn shift_makes_a_square_that_grows_away_from_the_press_corner() {
    let mut ed = armed(Tool::Rectangle);
    drag(&mut ed, at(300.0, 300.0), shift(at(280.0, 220.0)));
    assert_eq!(frame(&only_shape(&ed)), (220.0, 220.0, 80.0, 80.0));

    let mut ed = armed(Tool::Ellipse);
    drag(&mut ed, at(300.0, 300.0), shift(at(390.0, 310.0)));
    assert_eq!(frame(&only_shape(&ed)), (300.0, 300.0, 90.0, 90.0));
}

#[test]
fn shift_squares_every_box_tool_including_redaction_and_spotlight() {
    for tool in [Tool::Blur, Tool::Pixelate, Tool::Highlight] {
        let mut ed = armed(tool);
        drag(&mut ed, at(100.0, 100.0), shift(at(130.0, 160.0)));
        assert_eq!(
            frame(&only_shape(&ed)),
            (100.0, 100.0, 60.0, 60.0),
            "{tool:?}"
        );
    }
}

#[test]
fn alt_grows_the_box_from_the_press_point_as_its_centre() {
    let mut ed = armed(Tool::Rectangle);
    drag(&mut ed, at(300.0, 300.0), alt(at(340.0, 320.0)));
    assert_eq!(frame(&only_shape(&ed)), (260.0, 280.0, 80.0, 40.0));

    let mut ed = armed(Tool::Ellipse);
    drag(&mut ed, at(300.0, 300.0), shift(alt(at(340.0, 320.0))));
    assert_eq!(frame(&only_shape(&ed)), (260.0, 260.0, 80.0, 80.0));
}

#[test]
fn a_click_makes_a_default_square_scaled_to_the_image_at_the_press_point() {
    let mut ed = armed(Tool::FilledRectangle);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(400.0, 300.0), at(401.5, 301.0));
    let s = only_shape(&ed);
    assert_eq!(frame(&s), (400.0, 300.0, 144.0, 144.0));
    assert!(matches!(&s.kind, ShapeKind::Geo(p) if p.fill == FillStyle::Solid));
    one_undo_entry(&mut ed, &before);

    let mut ed = armed(Tool::Rectangle);
    drag(&mut ed, at(400.0, 300.0), at(403.0, 300.0));
    assert_eq!(
        frame(&only_shape(&ed)).2,
        3.0,
        "a 3 unit drag is not a click"
    );

    let mut ed = armed(Tool::Rectangle);
    click(&mut ed, alt(at(400.0, 300.0)));
    assert_eq!(frame(&only_shape(&ed)), (328.0, 228.0, 144.0, 144.0));
}

#[test]
fn the_click_default_never_drops_below_48_on_small_images() {
    let mut ed = armed(Tool::Rectangle);
    ed.set_viewport(es_anno::Viewport {
        image_size: es_geom::Size::new(300.0, 200.0),
        ..*ed.viewport()
    });
    click(&mut ed, at(10.0, 10.0));
    assert_eq!(frame(&only_shape(&ed)), (10.0, 10.0, 48.0, 48.0));
}

#[test]
fn redactions_take_their_kind_from_the_tool_and_the_current_density() {
    for (tool, kind) in [
        (Tool::Blur, RedactionKind::Blur),
        (Tool::Pixelate, RedactionKind::Pixelate),
    ] {
        let mut ed = armed(tool);
        let before = ed.doc().snapshot();
        drag(&mut ed, at(10.0, 10.0), at(110.0, 60.0));
        let s = only_shape(&ed);
        assert_eq!(frame(&s), (10.0, 10.0, 100.0, 50.0));
        let ShapeKind::Redaction(p) = &s.kind else {
            panic!("{tool:?} made {:?}", s.kind)
        };
        assert_eq!(p.kind, kind);
        assert_eq!(p.density, ed.current_style().redaction_density);
        one_undo_entry(&mut ed, &before);
    }
    let mut ed = armed(Tool::Highlight);
    drag(&mut ed, at(10.0, 10.0), at(110.0, 60.0));
    assert!(only_shape(&ed).is_highlight());
}

fn arrow_ends(s: &AnnoShape) -> (PVec, PVec) {
    match &s.kind {
        ShapeKind::Arrow(p) => (p.start, p.end),
        other => panic!("not an arrow: {other:?}"),
    }
}

#[test]
fn an_arrow_runs_from_the_press_to_the_pointer_with_the_current_heads() {
    let mut ed = armed(Tool::Arrow);
    let before = ed.doc().snapshot();
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(at(150.0, 130.0));
    assert_eq!(
        ed.cursor_for(Point::new(150.0, 130.0)),
        CursorKind::DrawArrow
    );
    ed.pointer_move(at(200.0, 140.0));
    ed.pointer_up(at(200.0, 140.0));
    let s = only_shape(&ed);
    assert_eq!((s.x, s.y), (100.0, 100.0));
    let (start, end) = arrow_ends(&s);
    assert_eq!((start.x, start.y, end.x, end.y), (0.0, 0.0, 100.0, 40.0));
    let ShapeKind::Arrow(p) = &s.kind else {
        unreachable!()
    };
    assert_eq!(
        (p.arrowhead_start, p.arrowhead_end),
        (
            ed.current_style().arrowhead_start,
            ed.current_style().arrowhead_end
        )
    );
    assert_eq!(p.arrowhead_end, Arrowhead::Arrow);
    assert!(ed.selected_ids().contains(&s.id));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_line_has_no_heads() {
    let mut ed = armed(Tool::Line);
    drag(&mut ed, at(0.0, 0.0), at(50.0, 0.0));
    let s = only_shape(&ed);
    assert_eq!(s.tool(), Tool::Line);
}

#[test]
fn shift_snaps_the_arrow_to_the_nearest_15_degrees() {
    let mut ed = armed(Tool::Arrow);
    drag(&mut ed, at(0.0, 0.0), shift(at(100.0, 5.0)));
    let (_, end) = arrow_ends(&only_shape(&ed));
    let len = (100.0f64 * 100.0 + 5.0 * 5.0).sqrt();
    assert!((end.x - len).abs() < 1e-9 && end.y.abs() < 1e-9, "{end:?}");

    let mut ed = armed(Tool::Arrow);
    drag(&mut ed, at(0.0, 0.0), shift(at(100.0, 32.0)));
    let (_, end) = arrow_ends(&only_shape(&ed));
    let angle = end.y.atan2(end.x).to_degrees();
    assert!((angle - 15.0).abs() < 1e-9, "{angle}");
}

#[test]
fn an_arrow_shorter_than_4_is_dropped_and_leaves_no_undo_entry() {
    let mut ed = armed(Tool::Arrow);
    drag(&mut ed, at(10.0, 10.0), at(13.0, 12.0));
    assert!(ed.doc().shapes().is_empty());
    assert!(!ed.can_undo());

    let mut ed = armed(Tool::Arrow);
    drag(&mut ed, at(10.0, 10.0), at(14.0, 10.0));
    assert_eq!(ed.doc().shapes().len(), 1, "exactly 4 is long enough");
}

fn points(s: &AnnoShape) -> Vec<(f64, f64, f64)> {
    match &s.kind {
        ShapeKind::Draw(p) => p.points.iter().map(|v| (v.x, v.y, v.z)).collect(),
        other => panic!("not a stroke: {other:?}"),
    }
}

#[test]
fn freehand_records_points_from_the_press_and_skips_ones_that_barely_move() {
    let mut ed = armed(Tool::Freehand);
    let before = ed.doc().snapshot();
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(at(110.0, 100.0));
    ed.pointer_move(at(110.2, 100.2));
    let (id, live) = ed.live_stroke().expect("a live stroke while drawing");
    assert_eq!(live.raw().len(), 2);
    assert_eq!(
        ed.cursor_for(Point::new(110.0, 100.0)),
        CursorKind::Pen,
        "the pen stays while drawing"
    );
    ed.pointer_move(at(120.0, 105.0));
    ed.pointer_up(at(120.0, 105.0));
    assert!(ed.live_stroke().is_none());
    let s = only_shape(&ed);
    assert_eq!(s.id, id);
    assert_eq!((s.x, s.y), (100.0, 100.0));
    assert_eq!(
        points(&s),
        vec![(0.0, 0.0, 0.5), (10.0, 0.0, 0.5), (20.0, 5.0, 0.5)]
    );
    let ShapeKind::Draw(p) = &s.kind else {
        unreachable!()
    };
    assert!(p.is_complete && !p.is_closed && !p.is_pen);
    assert!(ed.selected_ids().is_empty(), "a stroke is not selected");
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_pen_records_its_pressure() {
    let mut ed = armed(Tool::Freehand);
    let pen = |x, pressure| PointerInfo {
        is_pen: true,
        pressure,
        ..at(x, 0.0)
    };
    ed.pointer_down(pen(0.0, 0.2));
    ed.pointer_move(pen(10.0, 0.9));
    ed.pointer_up(pen(10.0, 0.9));
    let s = only_shape(&ed);
    assert_eq!(points(&s), vec![(0.0, 0.0, 0.2), (10.0, 0.0, 0.9)]);
    assert!(matches!(&s.kind, ShapeKind::Draw(p) if p.is_pen));
}

#[test]
fn a_stroke_that_returns_to_its_start_closes() {
    let mut ed = armed(Tool::Freehand);
    ed.pointer_down(at(100.0, 100.0));
    for (x, y) in [
        (200.0, 100.0),
        (200.0, 200.0),
        (100.0, 200.0),
        (101.0, 101.0),
    ] {
        ed.pointer_move(at(x, y));
    }
    ed.pointer_up(at(101.0, 101.0));
    assert!(matches!(&only_shape(&ed).kind, ShapeKind::Draw(p) if p.is_closed));

    let mut ed = armed(Tool::Freehand);
    ed.pointer_down(at(100.0, 100.0));
    for (x, y) in [(200.0, 100.0), (200.0, 200.0), (100.0, 200.0)] {
        ed.pointer_move(at(x, y));
    }
    ed.pointer_up(at(100.0, 200.0));
    assert!(matches!(&only_shape(&ed).kind, ShapeKind::Draw(p) if !p.is_closed));
}

#[test]
fn a_freehand_click_draws_nothing_and_leaves_no_undo_entry() {
    let mut ed = armed(Tool::Freehand);
    click(&mut ed, at(50.0, 50.0));
    assert!(ed.doc().shapes().is_empty());
    assert!(!ed.can_undo());
}

fn numbered(s: &AnnoShape) -> (u32, f64, f64, f64) {
    match &s.kind {
        ShapeKind::Numbered(p) => (p.value, p.diameter, s.x, s.y),
        other => panic!("not a counter: {other:?}"),
    }
}

#[test]
fn counters_number_themselves_and_sit_centred_on_the_click() {
    let mut ed = armed(Tool::NumberedCircle);
    let before = ed.doc().snapshot();
    click(&mut ed, at(500.0, 400.0));
    let d = 1800.0 * 0.039;
    let first = only_shape(&ed);
    assert_eq!(numbered(&first), (1, d, 500.0 - d / 2.0, 400.0 - d / 2.0));
    assert!(ed.selected_ids().contains(&first.id));
    one_undo_entry(&mut ed, &before);

    click(&mut ed, at(600.0, 400.0));
    click(&mut ed, at(700.0, 400.0));
    let values: Vec<u32> = ed.doc().shapes().iter().map(|s| numbered(s).0).collect();
    assert_eq!(values, vec![1, 2, 3]);
    assert_eq!(ed.next_numbered_value(), 4);
}

#[test]
fn deleting_the_highest_counter_frees_its_number_but_a_middle_one_leaves_a_gap() {
    let mut ed = armed(Tool::NumberedCircle);
    for x in [100.0, 200.0, 300.0] {
        click(&mut ed, at(x, 100.0));
    }
    let ids: Vec<_> = ed.doc().shapes().iter().map(|s| s.id).collect();
    ed.select([ids[2]]);
    ed.delete_selected();
    assert_eq!(ed.next_numbered_value(), 3);
    ed.select([ids[1]]);
    ed.delete_selected();
    assert_eq!(ed.next_numbered_value(), 2);
    ed.select([ids[0]]);
    ed.delete_selected();
    assert_eq!(ed.next_numbered_value(), 1);
}

#[test]
fn counters_never_shrink_below_24_on_small_images() {
    let mut ed = armed(Tool::NumberedCircle);
    ed.set_viewport(es_anno::Viewport {
        image_size: es_geom::Size::new(300.0, 200.0),
        ..*ed.viewport()
    });
    click(&mut ed, at(50.0, 50.0));
    assert_eq!(numbered(&only_shape(&ed)).1, 24.0);
}

#[test]
fn each_drawing_tool_shows_what_a_press_makes_even_over_existing_shapes() {
    let r = filled_rect(0.0, 0.0, 100.0, 100.0);
    let mut ed = editor(vec![r], vec![]);
    let families = [
        (Tool::Rectangle, CursorKind::DrawRectangle),
        (Tool::FilledRectangle, CursorKind::DrawRectangle),
        (Tool::Ellipse, CursorKind::DrawEllipse),
        (Tool::Line, CursorKind::DrawLine),
        (Tool::Arrow, CursorKind::DrawArrow),
        (Tool::Freehand, CursorKind::Pen),
        (Tool::Highlighter, CursorKind::Pen),
        (Tool::Text, CursorKind::IBeam),
        (Tool::NumberedCircle, CursorKind::Badge),
        (Tool::Highlight, CursorKind::Area),
        (Tool::Pixelate, CursorKind::Eraser),
        (Tool::Blur, CursorKind::Eraser),
    ];
    for (tool, want) in families {
        ed.select_tool(tool);
        assert_eq!(ed.cursor_for(Point::new(50.0, 50.0)), want, "{tool:?}");
        assert_eq!(ed.cursor_for(Point::new(500.0, 500.0)), want, "{tool:?}");
    }
}

#[test]
fn only_the_primary_button_creates() {
    let mut ed = armed(Tool::Rectangle);
    for button in [
        es_anno::PointerButton::Secondary,
        es_anno::PointerButton::Middle,
    ] {
        let p = PointerInfo {
            button,
            ..at(10.0, 10.0)
        };
        ed.pointer_down(p);
        ed.pointer_move(at(100.0, 100.0));
        ed.pointer_up(at(100.0, 100.0));
    }
    assert!(ed.doc().shapes().is_empty());
}

#[test]
fn undo_in_the_middle_of_a_drag_ends_the_drag() {
    let mut ed = armed(Tool::Rectangle);
    ed.pointer_down(at(10.0, 10.0));
    ed.pointer_move(at(100.0, 100.0));
    ed.undo();
    assert!(ed.is_idle());
    assert!(ed.doc().shapes().is_empty());
    ed.pointer_move(at(200.0, 200.0));
    ed.pointer_up(at(200.0, 200.0));
    assert!(ed.doc().shapes().is_empty());
}

#[test]
fn picked_heads_and_dash_reach_new_arrows_but_never_give_a_line_heads() {
    use es_anno::{GestureId, StylePatch};
    use es_model::DashStyle;

    let mut ed = armed(Tool::Line);
    ed.pointer_down(at(10.0, 10.0));
    ed.pointer_move(at(90.0, 10.0));
    ed.pointer_up(at(90.0, 10.0));
    ed.apply_style(StylePatch::ArrowheadEnd(Arrowhead::Diamond), GestureId(1));
    ed.apply_style(StylePatch::Dash(DashStyle::Dotted), GestureId(2));
    let ShapeKind::Arrow(line) = &only_shape(&ed).kind else {
        unreachable!()
    };
    assert_eq!(
        (line.arrowhead_end, line.dash),
        (Arrowhead::None, DashStyle::Dotted),
        "a selected line takes the dash and stays a line"
    );

    let mut ed = armed(Tool::Arrow);
    ed.apply_style(StylePatch::ArrowheadStart(Arrowhead::Dot), GestureId(1));
    ed.apply_style(StylePatch::Dash(DashStyle::Dashed), GestureId(2));
    ed.pointer_down(at(10.0, 10.0));
    ed.pointer_move(at(90.0, 10.0));
    ed.pointer_up(at(90.0, 10.0));
    let ShapeKind::Arrow(arrow) = &only_shape(&ed).kind else {
        unreachable!()
    };
    assert_eq!(
        (arrow.arrowhead_start, arrow.arrowhead_end, arrow.dash),
        (Arrowhead::Dot, Arrowhead::Arrow, DashStyle::Dashed)
    );
}
