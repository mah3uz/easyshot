use crate::common::*;
use es_anno::draw::DrawItem;
use es_anno::{BuildContext, Editor, PointerInfo, build_draw_list};
use es_model::{AnnoShape, ShapeKind, Tool};
use kurbo::Shape as _;

fn armed() -> Editor {
    let mut ed = editor(vec![], vec![]);
    ed.select_tool(Tool::Highlighter);
    ed
}

fn only_shape(ed: &Editor) -> AnnoShape {
    assert_eq!(ed.doc().shapes().len(), 1, "exactly one shape");
    ed.doc().shapes()[0].clone()
}

fn pts(s: &AnnoShape) -> Vec<(f64, f64)> {
    match &s.kind {
        ShapeKind::Highlighter(p) => p.points.iter().map(|q| (q.x, q.y)).collect(),
        other => panic!("not a highlighter: {other:?}"),
    }
}

fn shifted(x: f64, y: f64) -> PointerInfo {
    PointerInfo {
        shift: true,
        ..at(x, y)
    }
}

#[test]
fn m_again_switches_between_freehand_and_straight_and_keeps_the_tool() {
    let mut ed = armed();
    assert!(!ed.highlighter_straight());
    let ch = ed.select_tool(Tool::Highlighter);
    assert!(ed.highlighter_straight() && ed.tool() == Tool::Highlighter);
    assert!(ch.tool_changed);
    ed.select_tool(Tool::Highlighter);
    assert!(!ed.highlighter_straight());
}

#[test]
fn freehand_keeps_every_sampled_point_as_one_undo_step() {
    let mut ed = armed();
    let before = ed.doc().snapshot();
    ed.pointer_down(at(100.0, 100.0));
    for (x, y) in [(120.0, 104.0), (150.0, 98.0), (200.0, 101.0)] {
        ed.pointer_move(at(x, y));
    }
    ed.pointer_up(at(200.0, 101.0));
    let s = only_shape(&ed);
    assert_eq!((s.x, s.y), (100.0, 100.0));
    assert_eq!(
        pts(&s),
        vec![(0.0, 0.0), (20.0, 4.0), (50.0, -2.0), (100.0, 1.0)]
    );
    one_undo_entry(&mut ed, &before);
}

#[test]
fn straight_mode_keeps_only_the_press_and_the_latest_point() {
    let mut ed = armed();
    ed.select_tool(Tool::Highlighter);
    ed.pointer_down(at(100.0, 100.0));
    for (x, y) in [(120.0, 150.0), (160.0, 90.0), (300.0, 130.0)] {
        ed.pointer_move(at(x, y));
    }
    ed.pointer_up(at(300.0, 130.0));
    assert_eq!(pts(&only_shape(&ed)), vec![(0.0, 0.0), (200.0, 30.0)]);
}

#[test]
fn shift_snaps_a_straight_stroke_to_fifteen_degrees() {
    let mut ed = armed();
    ed.select_tool(Tool::Highlighter);
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(shifted(300.0, 108.0));
    ed.pointer_up(shifted(300.0, 108.0));
    let end = pts(&only_shape(&ed))[1];
    assert!(end.1.abs() < 1e-9, "snapped flat, got {end:?}");
    assert!((end.0 - 200.0_f64.hypot(8.0)).abs() < 1e-6);

    let mut ed = armed();
    ed.select_tool(Tool::Highlighter);
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(shifted(300.0, 300.0));
    ed.pointer_up(shifted(300.0, 300.0));
    let (x, y) = pts(&only_shape(&ed))[1];
    assert!((x - y).abs() < 1e-6, "45 degrees, got {x} {y}");
}

#[test]
fn freehand_ignores_shift() {
    let mut ed = armed();
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(shifted(300.0, 108.0));
    ed.pointer_up(shifted(300.0, 108.0));
    assert_eq!(pts(&only_shape(&ed))[1], (200.0, 8.0));
}

#[test]
fn the_stroke_stays_on_the_image() {
    let mut ed = armed();
    ed.pointer_down(at(1700.0, 500.0));
    ed.pointer_move(at(5000.0, 500.0));
    ed.pointer_up(at(5000.0, 500.0));
    let s = only_shape(&ed);
    assert_eq!(pts(&s)[1].0 + s.x, 1800.0);
}

#[test]
fn a_click_without_a_drag_leaves_nothing() {
    let mut ed = armed();
    ed.pointer_down(at(50.0, 50.0));
    ed.pointer_up(at(50.0, 50.0));
    assert!(ed.doc().shapes().is_empty());
}

#[test]
fn the_draw_list_paints_it_below_ordinary_shapes_in_page_space() {
    let mut ed = armed();
    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(at(200.0, 100.0));
    ed.pointer_up(at(200.0, 100.0));
    let rect = filled_rect(0.0, 0.0, 50.0, 50.0);
    let mut shapes = ed.doc().shapes().to_vec();
    shapes.push(rect);
    let ed = editor(shapes, vec![]);
    let list = build_draw_list(ed.doc(), &BuildContext::new(&FakeOutliner));
    assert!(matches!(list.items[0], DrawItem::Highlighter { .. }));
    let DrawItem::Highlighter { path, color, .. } = &list.items[0] else {
        unreachable!()
    };
    assert_eq!(path.bounding_box().x0, 100.0);
    assert!((color.a - 0.4).abs() < 1e-6);
}
