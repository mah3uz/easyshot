mod common;

use common::*;
use es_anno::{Editor, Handle, PointerInfo};
use es_geom::{PVec, Point};
use es_model::{
    AnnoShape, AnnoShapeId, ArrowBinding, ArrowTerminal, HighlightProps, ShapeKind, Tool,
};

fn alt(p: PointerInfo) -> PointerInfo {
    PointerInfo { alt: true, ..p }
}

fn ends(ed: &Editor, id: AnnoShapeId) -> (PVec, PVec, f64) {
    match &ed.doc().shape(id).expect("arrow in document").kind {
        ShapeKind::Arrow(p) => (p.start, p.end, p.bend),
        other => panic!("not an arrow: {other:?}"),
    }
}

fn near(a: PVec, b: (f64, f64)) -> bool {
    (a.x - b.0).abs() < 1e-6 && (a.y - b.1).abs() < 1e-6
}

fn binding(ed: &Editor, id: AnnoShapeId, terminal: ArrowTerminal) -> Option<ArrowBinding> {
    ed.doc()
        .bindings()
        .iter()
        .find(|b| b.arrow_id == id && b.terminal == terminal)
        .cloned()
}

/// A selected arrow at (100, 100) running 200 to the right, and a filled box centred at
/// (450, 100).
fn arrow_and_target() -> (Editor, AnnoShapeId, AnnoShapeId) {
    let a = arrow(100.0, 100.0, (0.0, 0.0), (200.0, 0.0));
    let t = filled_rect(400.0, 50.0, 100.0, 100.0);
    let (ai, ti) = (a.id, t.id);
    let mut ed = editor(vec![a, t], vec![]);
    select(&mut ed, &[ai]);
    (ed, ai, ti)
}

#[test]
fn a_lone_arrow_has_start_bend_and_end_handles_and_no_frame() {
    let (ed, _, _) = arrow_and_target();
    assert_eq!(
        ed.handle_at(Point::new(100.0, 100.0)),
        Some(Handle::ArrowStart)
    );
    assert_eq!(
        ed.handle_at(Point::new(200.0, 100.0)),
        Some(Handle::ArrowMiddle)
    );
    assert_eq!(
        ed.handle_at(Point::new(300.0, 100.0)),
        Some(Handle::ArrowEnd)
    );
    assert_eq!(ed.handle_at(Point::new(90.0, 90.0)), None, "no rotate zone");
}

#[test]
fn dropping_an_end_on_a_shape_binds_it_to_the_shape_centre_as_one_undo_step() {
    let (mut ed, ai, ti) = arrow_and_target();
    let before = ed.doc().snapshot();
    ed.pointer_down(at(300.0, 100.0));
    ed.pointer_move(at(430.0, 80.0));
    assert_eq!(
        ed.hinted_binding_id(),
        Some(ti),
        "the target is shown while hovering"
    );
    ed.pointer_up(at(430.0, 80.0));
    assert_eq!(ed.hinted_binding_id(), None);
    let b = binding(&ed, ai, ArrowTerminal::End).expect("the end is bound");
    assert_eq!(b.to_id, ti);
    assert!(!b.is_precise && !b.is_exact);
    assert!((b.normalized_anchor.x - 0.3).abs() < 1e-9);
    assert!((b.normalized_anchor.y - 0.3).abs() < 1e-9);
    assert!(near(ends(&ed, ai).1, (350.0, 0.0)));
    assert!(binding(&ed, ai, ArrowTerminal::Start).is_none());
    one_undo_entry(&mut ed, &before);
}

#[test]
fn alt_binds_precisely_to_the_point_under_the_pointer() {
    let (mut ed, ai, ti) = arrow_and_target();
    drag(&mut ed, at(300.0, 100.0), alt(at(430.0, 80.0)));
    let b = binding(&ed, ai, ArrowTerminal::End).expect("the end is bound");
    assert_eq!(b.to_id, ti);
    assert!(b.is_precise);
    assert!(near(ends(&ed, ai).1, (330.0, -20.0)));
}

#[test]
fn the_start_handle_binds_too() {
    let (mut ed, ai, ti) = arrow_and_target();
    drag(&mut ed, at(100.0, 100.0), at(420.0, 120.0));
    let b = binding(&ed, ai, ArrowTerminal::Start).expect("the start is bound");
    assert_eq!(b.to_id, ti);
    assert!(near(ends(&ed, ai).0, (350.0, 0.0)));
}

#[test]
fn dragging_a_bound_end_off_its_shape_unbinds_it_and_leaves_it_at_the_pointer() {
    let a = arrow(100.0, 100.0, (0.0, 0.0), (200.0, 0.0));
    let t = filled_rect(400.0, 50.0, 100.0, 100.0);
    let (ai, ti) = (a.id, t.id);
    let mut ed = editor(vec![a, t], vec![bind(ai, ti, ArrowTerminal::End)]);
    select(&mut ed, &[ai]);
    assert!(near(ends(&ed, ai).1, (350.0, 0.0)));
    let before = ed.doc().snapshot();
    ed.pointer_down(at(450.0, 100.0));
    ed.pointer_move(at(300.0, 300.0));
    assert_eq!(ed.hinted_binding_id(), None);
    ed.pointer_up(at(300.0, 300.0));
    assert!(binding(&ed, ai, ArrowTerminal::End).is_none());
    assert!(near(ends(&ed, ai).1, (200.0, 200.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn spotlights_redactions_and_arrows_do_not_catch_an_end_but_a_hollow_box_does_inside() {
    let a = arrow(100.0, 100.0, (0.0, 0.0), (200.0, 0.0));
    let ai = a.id;
    let spot = AnnoShape::new(
        400.0,
        50.0,
        ShapeKind::Highlight(HighlightProps { w: 100.0, h: 100.0 }),
    );
    let other = arrow(400.0, 300.0, (0.0, 0.0), (100.0, 0.0));
    let hollow = rect(700.0, 50.0, 100.0, 100.0);
    let hi = hollow.id;
    let mut ed = editor(vec![a, spot, other, hollow], vec![]);
    select(&mut ed, &[ai]);
    drag(&mut ed, at(300.0, 100.0), at(450.0, 100.0));
    assert!(binding(&ed, ai, ArrowTerminal::End).is_none());
    drag(&mut ed, at(450.0, 100.0), at(450.0, 300.0));
    assert!(binding(&ed, ai, ArrowTerminal::End).is_none());
    drag(&mut ed, at(450.0, 300.0), at(750.0, 100.0));
    assert_eq!(
        binding(&ed, ai, ArrowTerminal::End).map(|b| b.to_id),
        Some(hi)
    );
}

#[test]
fn the_bend_handle_bends_across_the_chord_and_snaps_straight_below_8() {
    let (mut ed, ai, _) = arrow_and_target();
    let before = ed.doc().snapshot();
    drag(&mut ed, at(200.0, 100.0), at(200.0, 130.0));
    assert!((ends(&ed, ai).2 - 30.0).abs() < 1e-9);
    let middle = ed.doc().arrow_info(ai).expect("arrow").middle;
    assert!(
        near(middle, (100.0, 30.0)),
        "the handle stays under the pointer"
    );
    one_undo_entry(&mut ed, &before);

    drag(&mut ed, at(200.0, 130.0), at(200.0, 109.0));
    assert!((ends(&ed, ai).2 - 9.0).abs() < 1e-9);
    drag(&mut ed, at(200.0, 109.0), at(200.0, 107.0));
    assert_eq!(ends(&ed, ai).2, 0.0);
    drag(&mut ed, at(200.0, 100.0), at(200.0, 92.0));
    assert!(
        (ends(&ed, ai).2 + 8.0).abs() < 1e-9,
        "the other side bends negative"
    );
}

#[test]
fn a_new_arrow_binds_its_start_to_the_shape_it_was_drawn_from() {
    let t = filled_rect(400.0, 50.0, 100.0, 100.0);
    let ti = t.id;
    let mut ed = editor(vec![t], vec![]);
    ed.select_tool(Tool::Arrow);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(420.0, 70.0), at(700.0, 100.0));
    let ai = ed.doc().shapes()[1].id;
    assert_eq!(
        binding(&ed, ai, ArrowTerminal::Start).map(|b| b.to_id),
        Some(ti)
    );
    assert!(binding(&ed, ai, ArrowTerminal::End).is_none());
    let (start, end, _) = ends(&ed, ai);
    assert!(near(start, (30.0, 30.0)), "the start moves to the centre");
    assert!(near(end, (280.0, 30.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_new_arrow_binds_its_end_to_the_shape_it_ends_on() {
    let t = filled_rect(800.0, 50.0, 100.0, 100.0);
    let ti = t.id;
    let mut ed = editor(vec![t], vec![]);
    ed.select_tool(Tool::Arrow);
    ed.pointer_down(at(600.0, 300.0));
    ed.pointer_move(at(820.0, 70.0));
    assert_eq!(ed.hinted_binding_id(), Some(ti));
    ed.pointer_move(at(700.0, 300.0));
    assert_eq!(ed.hinted_binding_id(), None, "moving off clears the hint");
    ed.pointer_move(at(820.0, 70.0));
    ed.pointer_up(at(820.0, 70.0));
    assert_eq!(ed.hinted_binding_id(), None);
    let ai = ed.doc().shapes()[1].id;
    assert_eq!(
        binding(&ed, ai, ArrowTerminal::End).map(|b| b.to_id),
        Some(ti)
    );
    assert!(near(ends(&ed, ai).1, (250.0, -200.0)));
}

#[test]
fn a_click_with_the_arrow_tool_on_a_shape_leaves_no_arrow() {
    let t = filled_rect(400.0, 50.0, 100.0, 100.0);
    let mut ed = editor(vec![t], vec![]);
    ed.select_tool(Tool::Arrow);
    ed.pointer_down(at(420.0, 70.0));
    ed.pointer_up(at(420.0, 70.0));
    assert_eq!(ed.doc().shapes().len(), 1);
    assert!(ed.doc().bindings().is_empty());
    assert!(!ed.can_undo());
}
