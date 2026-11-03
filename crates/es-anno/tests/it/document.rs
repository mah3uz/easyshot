use crate::common;

use common::*;
use es_anno::Document;
use es_geom::{PVec, Point};
use es_model::{ArrowTerminal, ShapeKind};
use std::sync::Arc;

fn arrow_end(doc: &Document, id: es_model::AnnoShapeId) -> PVec {
    match &doc.shape(id).expect("shape in document").kind {
        ShapeKind::Arrow(p) => p.end,
        _ => panic!("not an arrow"),
    }
}

fn close(a: PVec, b: PVec) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn moving_a_shape_rebuilds_only_itself_and_the_arrows_bound_to_it() {
    let target = rect(200.0, 0.0, 100.0, 100.0);
    let bystander = rect(600.0, 600.0, 50.0, 50.0);
    let a = arrow(0.0, 0.0, (0.0, 50.0), (100.0, 50.0));
    let (t, b, ar) = (target.id, bystander.id, a.id);
    let mut ed = editor(
        vec![target, bystander, a],
        vec![bind(ar, t, ArrowTerminal::End)],
    );

    let arrow_geometry = ed.doc().geometry(ar).unwrap();
    let bystander_geometry = ed.doc().geometry(b).unwrap();
    let (arrow_rev, bystander_rev) = (ed.doc().revision(ar), ed.doc().revision(b));

    select(&mut ed, &[t]);
    let ch = ed.nudge_selected(10.0, 0.0, g(1));

    assert_eq!(
        ch.dirty_shapes.as_slice(),
        handles(&ed, &[t, ar]).as_slice()
    );
    assert_ne!(ed.doc().revision(ar), arrow_rev);
    assert!(!Arc::ptr_eq(
        &arrow_geometry,
        &ed.doc().geometry(ar).unwrap()
    ));
    assert_eq!(ed.doc().revision(b), bystander_rev);
    assert!(Arc::ptr_eq(
        &bystander_geometry,
        &ed.doc().geometry(b).unwrap()
    ));
}

#[test]
fn an_unchanged_shape_serves_the_same_cached_geometry() {
    let ed = editor(vec![rect(0.0, 0.0, 10.0, 10.0)], vec![]);
    let id = ed.doc().shapes()[0].id;
    let first = ed.doc().geometry(id).unwrap();
    assert!(Arc::ptr_eq(&first, &ed.doc().geometry(id).unwrap()));
}

#[test]
fn a_bound_terminal_is_written_back_so_a_document_without_bindings_draws_the_same() {
    let target = rect(200.0, 0.0, 100.0, 100.0);
    let a = arrow(0.0, 0.0, (0.0, 50.0), (100.0, 50.0));
    let (t, ar) = (target.id, a.id);
    let mut ed = editor(vec![target, a], vec![bind(ar, t, ArrowTerminal::End)]);
    assert!(close(arrow_end(ed.doc(), ar), PVec::new(250.0, 50.0)));

    select(&mut ed, &[t]);
    ed.nudge_selected(10.0, 20.0, g(1));
    let bound = ed.doc().arrow_info(ar).unwrap();
    assert!(close(arrow_end(ed.doc(), ar), PVec::new(260.0, 70.0)));

    let mut exported = ed.doc().model().clone();
    exported.bindings.clear();
    let unbound = Document::new(exported, Box::new(FakeMeasure));
    assert!(close(
        unbound.arrow_info(ar).unwrap().end.handle,
        bound.end.handle
    ));
}

#[test]
fn a_loaded_document_with_stale_terminals_is_synced_on_open() {
    let target = rect(200.0, 0.0, 100.0, 100.0);
    let a = arrow(0.0, 0.0, (0.0, 50.0), (0.0, 0.0));
    let (t, ar) = (target.id, a.id);
    let doc = Document::new(
        model(vec![target, a], vec![bind(ar, t, ArrowTerminal::End)]),
        Box::new(FakeMeasure),
    );
    assert!(close(arrow_end(&doc, ar), PVec::new(250.0, 50.0)));
}

#[test]
fn deleting_the_target_leaves_the_arrow_where_it_was_drawn() {
    let target = rect(200.0, 0.0, 100.0, 100.0);
    let a = arrow(0.0, 0.0, (0.0, 50.0), (100.0, 50.0));
    let (t, ar) = (target.id, a.id);
    let mut ed = editor(vec![target, a], vec![bind(ar, t, ArrowTerminal::End)]);
    select(&mut ed, &[t]);
    let ch = ed.delete_selected();
    assert!(ed.doc().bindings().is_empty());
    assert!(close(arrow_end(ed.doc(), ar), PVec::new(250.0, 50.0)));
    assert!(ch.dirty_shapes.contains(&ed.doc().handle(ar).unwrap()));
}

#[test]
fn bindings_to_arrows_or_to_itself_resolve_as_unbound_instead_of_recursing() {
    let a = arrow(0.0, 0.0, (0.0, 0.0), (100.0, 0.0));
    let b = arrow(0.0, 100.0, (0.0, 0.0), (100.0, 0.0));
    let (ai, bi) = (a.id, b.id);
    let doc = Document::new(
        model(
            vec![a, b],
            vec![
                bind(ai, bi, ArrowTerminal::End),
                bind(bi, ai, ArrowTerminal::End),
                bind(ai, ai, ArrowTerminal::Start),
            ],
        ),
        Box::new(FakeMeasure),
    );
    assert!(close(arrow_end(&doc, ai), PVec::new(100.0, 0.0)));
    assert!(doc.geometry(ai).is_some() && doc.geometry(bi).is_some());
}

#[test]
fn duplicate_ids_in_a_loaded_file_are_made_unique() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let mut b = rect(50.0, 0.0, 10.0, 10.0);
    b.id = a.id;
    let doc = Document::new(model(vec![a, b], vec![]), Box::new(FakeMeasure));
    let (first, second) = (doc.shapes()[0].id, doc.shapes()[1].id);
    assert_ne!(first, second);
    assert_eq!(doc.shape(second).map(|s| s.x), Some(50.0));
}

#[test]
fn a_shape_keeps_its_handle_through_deletion_and_undo() {
    let r = rect(0.0, 0.0, 10.0, 10.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    let handle = ed.doc().handle(id);
    select(&mut ed, &[id]);
    ed.delete_selected();
    assert_eq!(ed.doc().handle(id), handle);
    ed.undo();
    assert_eq!(ed.doc().handle(id), handle);
    assert_eq!(ed.doc().id_of(handle.unwrap()), Some(id));
}

#[test]
fn hollow_shapes_are_hit_on_the_outline_and_filled_ones_anywhere() {
    let hollow = rect(0.0, 0.0, 200.0, 200.0);
    let filled = filled_rect(300.0, 0.0, 200.0, 200.0);
    let (h, f) = (hollow.id, filled.id);
    let ed = editor(vec![hollow, filled], vec![]);
    assert_eq!(ed.hit_shape(Point::new(100.0, 100.0)), None);
    assert_eq!(ed.hit_shape(Point::new(3.0, 100.0)), Some(h));
    assert_eq!(ed.hit_shape(Point::new(400.0, 100.0)), Some(f));
}

#[test]
fn the_topmost_shape_wins_a_hit() {
    let below = filled_rect(0.0, 0.0, 100.0, 100.0);
    let above = filled_rect(50.0, 50.0, 100.0, 100.0);
    let top = above.id;
    let ed = editor(vec![below, above], vec![]);
    assert_eq!(ed.hit_shape(Point::new(75.0, 75.0)), Some(top));
}

#[test]
fn the_hit_margin_stays_eight_screen_pixels_when_zoomed_out() {
    let mut ed = editor(vec![], vec![]);
    assert_eq!(ed.hit_margin(), 8.0);
    let mut v = *ed.viewport();
    v.scale = 0.5;
    ed.set_viewport(v);
    assert_eq!(ed.hit_margin(), 16.0);
    v.scale = 4.0;
    ed.set_viewport(v);
    assert_eq!(ed.hit_margin(), 4.0);
}

#[test]
fn text_geometry_is_the_measured_box() {
    let t = text(10.0, 10.0, "hello");
    let id = t.id;
    let ed = editor(vec![t], vec![]);
    let b = ed.doc().geometry(id).unwrap().bounds();
    assert_eq!((b.w, b.h), (51.0, 20.0 * 1.35));
}

#[test]
fn a_selected_rotated_shape_keeps_its_own_frame() {
    let mut r = rect(100.0, 100.0, 40.0, 20.0);
    r.rotation = std::f64::consts::FRAC_PI_2;
    let other = rect(0.0, 0.0, 10.0, 10.0);
    let (ri, oi) = (r.id, other.id);
    let mut ed = editor(vec![r, other], vec![]);
    select(&mut ed, &[ri]);
    let one = ed.selection_bounds().unwrap();
    assert_eq!((one.bbox.w, one.bbox.h), (40.0, 20.0));
    select(&mut ed, &[ri, oi]);
    let both = ed.selection_bounds().unwrap();
    assert_eq!(both.transform, es_geom::Mat::IDENTITY);
    assert!((both.bbox.max_y() - 140.0).abs() < 1e-9);
}

#[test]
fn adding_a_shape_whose_id_is_taken_is_refused() {
    let r = rect(0.0, 0.0, 10.0, 10.0);
    let id = r.id;
    let mut doc = Document::new(model(vec![r], vec![]), Box::new(FakeMeasure));
    let mut clash = rect(50.0, 0.0, 10.0, 10.0);
    clash.id = id;
    assert!(doc.add(clash).is_empty());
    assert_eq!(doc.shapes().len(), 1);
    assert_eq!(doc.shape(id).map(|s| s.x), Some(0.0));
}
