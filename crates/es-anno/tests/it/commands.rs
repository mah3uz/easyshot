use crate::common;

use common::*;
use es_anno::editor::COPY_OFFSET;
use es_anno::{Editor, StylePatch, Viewport};
use es_geom::Point;
use es_model::{AnnoShapeId, ArrowTerminal, ShapeKind, Swatch, Tool};
use std::collections::HashSet;

fn pos(ed: &Editor, id: AnnoShapeId) -> (f64, f64) {
    let s = ed.doc().shape(id).expect("shape in document");
    (s.x, s.y)
}

#[test]
fn duplicate_moves_bindings_between_copies_and_drops_the_rest() {
    let left = rect(0.0, 0.0, 100.0, 100.0);
    let right = rect(400.0, 0.0, 100.0, 100.0);
    let a = arrow(0.0, 0.0, (50.0, 50.0), (450.0, 50.0));
    let (l, r, ar) = (left.id, right.id, a.id);
    let mut ed = editor(
        vec![left, right, a],
        vec![
            bind(ar, l, ArrowTerminal::Start),
            bind(ar, r, ArrowTerminal::End),
        ],
    );
    select(&mut ed, &[l, ar]);
    ed.duplicate_selected();

    let copies: Vec<AnnoShapeId> = ed.selected_ids().iter().copied().collect();
    assert_eq!(copies.len(), 2);
    let copy_left = *copies
        .iter()
        .find(|id| !ed.doc().shape(**id).unwrap().is_arrow())
        .unwrap();
    let copy_arrow = *copies
        .iter()
        .find(|id| ed.doc().shape(**id).unwrap().is_arrow())
        .unwrap();
    assert_eq!(pos(&ed, copy_left), (COPY_OFFSET, COPY_OFFSET));

    let start = ed
        .doc()
        .model()
        .binding(copy_arrow, ArrowTerminal::Start)
        .unwrap();
    assert_eq!(
        start.to_id, copy_left,
        "the copy binds to the copied target"
    );
    assert!(
        ed.doc()
            .model()
            .binding(copy_arrow, ArrowTerminal::End)
            .is_none()
    );
    assert_eq!(
        ed.doc().bindings().len(),
        3,
        "the originals keep their bindings"
    );
}

#[test]
fn duplicated_shapes_get_fresh_ids() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let id = a.id;
    let mut ed = editor(vec![a], vec![]);
    select(&mut ed, &[id]);
    ed.duplicate_selected();
    ed.duplicate_selected();
    let unique: HashSet<_> = order(&ed).into_iter().collect();
    assert_eq!(unique.len(), 3);
}

#[test]
fn z_order_never_moves_a_redaction_or_a_spotlight() {
    let (red, a, spot, b, c) = (
        redaction(0.0, 0.0),
        rect(0.0, 0.0, 10.0, 10.0),
        spotlight(0.0, 0.0),
        rect(0.0, 0.0, 10.0, 10.0),
        rect(0.0, 0.0, 10.0, 10.0),
    );
    let ids = [red.id, a.id, spot.id, b.id, c.id];
    let [ri, ai, si, bi, ci] = ids;
    let mut ed = editor(vec![red, a, spot, b, c], vec![]);

    select(&mut ed, &[ri, ai, si]);
    ed.bring_to_front();
    assert_eq!(order(&ed), [ri, bi, si, ci, ai]);

    ed.send_to_back();
    assert_eq!(order(&ed), [ri, ai, si, bi, ci]);

    select(&mut ed, &[ri, si]);
    ed.bring_to_front();
    ed.send_to_back();
    assert_eq!(order(&ed), [ri, ai, si, bi, ci]);
}

#[test]
fn one_step_moves_past_one_shape_and_keeps_a_selected_run_together() {
    let shapes: Vec<_> = (0..4)
        .map(|i| rect(f64::from(i), 0.0, 10.0, 10.0))
        .collect();
    let ids: Vec<_> = shapes.iter().map(|s| s.id).collect();
    let mut ed = editor(shapes, vec![]);
    select(&mut ed, &[ids[0], ids[1]]);
    ed.bring_forward();
    assert_eq!(order(&ed), [ids[2], ids[0], ids[1], ids[3]]);
    ed.bring_forward();
    ed.bring_forward();
    assert_eq!(order(&ed), [ids[2], ids[3], ids[0], ids[1]]);
    ed.send_backward();
    assert_eq!(order(&ed), [ids[2], ids[0], ids[1], ids[3]]);
}

#[test]
fn tab_cycles_through_shapes_in_z_order_and_wraps() {
    let shapes: Vec<_> = (0..3)
        .map(|i| rect(f64::from(i), 0.0, 10.0, 10.0))
        .collect();
    let ids: Vec<_> = shapes.iter().map(|s| s.id).collect();
    let mut ed = editor(shapes, vec![]);
    let current = |ed: &Editor| ed.selected_ids().iter().copied().collect::<Vec<_>>();

    ed.select_next();
    assert_eq!(current(&ed), [ids[0]]);
    ed.select_next();
    ed.select_next();
    assert_eq!(current(&ed), [ids[2]]);
    ed.select_next();
    assert_eq!(current(&ed), [ids[0]]);
    ed.select_prev();
    assert_eq!(current(&ed), [ids[2]]);

    let mut ed = editor(
        vec![rect(0.0, 0.0, 1.0, 1.0), rect(0.0, 0.0, 1.0, 1.0)],
        vec![],
    );
    ed.select_prev();
    assert_eq!(current(&ed), [order(&ed)[1]]);
}

#[test]
fn tab_selection_switches_to_the_select_tool_and_shows_the_shapes_style() {
    let mut r = rect(0.0, 0.0, 10.0, 10.0);
    if let ShapeKind::Geo(p) = &mut r.kind {
        p.swatch = Swatch::green();
        p.stroke_width = 20.0;
    }
    let mut ed = editor(vec![r], vec![]);
    ed.select_tool(Tool::Ellipse);
    let ch = ed.select_next();
    assert!(ch.tool_changed && ch.selection_changed);
    assert_eq!(ed.tool(), Tool::Select);
    assert_eq!(ed.current_style().swatch, Swatch::green());
    assert_eq!(ed.current_style().stroke_width, 10.0);
}

#[test]
fn copy_takes_shapes_only_from_the_select_tool() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let id = a.id;
    let mut ed = editor(vec![a], vec![]);
    ed.copy_shapes();
    assert!(!ed.has_clipboard(), "nothing selected");
    select(&mut ed, &[id]);
    ed.select_tool(Tool::Rectangle);
    select(&mut ed, &[id]);
    ed.copy_shapes();
    assert!(!ed.has_clipboard(), "not the select tool");
    ed.select_tool(Tool::Select);
    select(&mut ed, &[id]);
    ed.copy_shapes();
    assert!(ed.has_clipboard());
}

#[test]
fn repeated_pastes_cascade_and_never_reuse_an_id() {
    let a = rect(100.0, 100.0, 10.0, 10.0);
    let id = a.id;
    let mut ed = editor(vec![a], vec![]);
    select(&mut ed, &[id]);
    ed.copy_shapes();
    ed.paste_shapes(None);
    let first = *ed.selected_ids().first().unwrap();
    ed.paste_shapes(None);
    let second = *ed.selected_ids().first().unwrap();
    assert_eq!(pos(&ed, first), (116.0, 116.0));
    assert_eq!(pos(&ed, second), (132.0, 132.0));
    assert_eq!(order(&ed).into_iter().collect::<HashSet<_>>().len(), 3);
}

#[test]
fn paste_at_the_pointer_centres_the_group_there_and_keeps_its_bindings() {
    let target = rect(0.0, 0.0, 100.0, 100.0);
    let a = arrow(0.0, 0.0, (300.0, 50.0), (0.0, 0.0));
    let (t, ar) = (target.id, a.id);
    let mut ed = editor(vec![target, a], vec![bind(ar, t, ArrowTerminal::End)]);
    select(&mut ed, &[t, ar]);
    ed.copy_shapes();
    ed.paste_shapes(Some(Point::new(1000.0, 500.0)));

    let pasted: Vec<_> = ed.selected_ids().iter().copied().collect();
    let boxes: Vec<_> = pasted
        .iter()
        .map(|id| ed.doc().page_bounds(*id).unwrap())
        .collect();
    let c = es_geom::BBox::common(&boxes).center();
    assert!((c.x - 1000.0).abs() < 1e-6 && (c.y - 500.0).abs() < 1e-6);
    assert_eq!(ed.doc().bindings().len(), 2);
}

#[test]
fn picking_the_active_tool_again_returns_to_select() {
    let mut ed = editor(vec![], vec![]);
    ed.select_tool(Tool::Arrow);
    assert_eq!(ed.tool(), Tool::Arrow);
    ed.select_tool(Tool::Arrow);
    assert_eq!(ed.tool(), Tool::Select);
    let ch = ed.select_tool(Tool::Select);
    assert_eq!(ed.tool(), Tool::Select);
    assert_eq!(ch, changes(|_| {}));
}

#[test]
fn style_reaches_only_shapes_that_have_that_property() {
    let r = rect(0.0, 0.0, 10.0, 10.0);
    let red = redaction(0.0, 0.0);
    let t = text(0.0, 0.0, "a");
    let (ri, di, ti) = (r.id, red.id, t.id);
    let mut ed = editor(vec![r, red, t], vec![]);
    select(&mut ed, &[ri, di, ti]);
    ed.apply_style(StylePatch::Swatch(Swatch::blue()), g(1));
    ed.apply_style(StylePatch::StrokeWidth(6.0), g(2));
    ed.apply_style(StylePatch::RedactionDensity(0.9), g(3));
    ed.apply_style(StylePatch::Bold(false), g(4));

    let doc = ed.doc();
    match (
        &doc.shape(ri).unwrap().kind,
        &doc.shape(di).unwrap().kind,
        &doc.shape(ti).unwrap().kind,
    ) {
        (ShapeKind::Geo(g), ShapeKind::Redaction(d), ShapeKind::Text(t)) => {
            assert_eq!((g.swatch.clone(), g.stroke_width), (Swatch::blue(), 12.0));
            assert_eq!(d.density, 0.9);
            assert_eq!(t.swatch, Swatch::blue());
            assert!(
                t.is_bold,
                "text style needs exactly one text shape selected"
            );
        }
        _ => panic!("kinds changed"),
    }
    assert!(
        !ed.current_style().bold,
        "the next text still takes the new style"
    );

    select(&mut ed, &[ti]);
    ed.apply_style(StylePatch::FontSize(1.0), g(5));
    match &ed.doc().shape(ti).unwrap().kind {
        ShapeKind::Text(t) => assert_eq!(t.font_size, 4.0),
        _ => panic!("not text"),
    }
}

#[test]
fn non_finite_input_never_reaches_the_document() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let id = a.id;
    let mut ed = editor(vec![a], vec![]);
    select(&mut ed, &[id]);
    ed.nudge_selected(f64::NAN, 0.0, g(1));
    ed.apply_style(StylePatch::StrokeWidth(f64::INFINITY), g(2));
    ed.apply_style(StylePatch::FontSize(f64::NAN), g(3));
    assert!(!ed.can_undo());
    assert_eq!(ed.current_style().stroke_width, 4.0);
}

#[test]
fn each_call_reports_exactly_what_changed() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let b = rect(50.0, 0.0, 10.0, 10.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    let ha = handles(&ed, &[ai]);

    assert_eq!(
        ed.select_tool(Tool::Rectangle),
        changes(|c| c.tool_changed = true)
    );
    assert_eq!(
        ed.select_all(),
        changes(|c| {
            c.tool_changed = true;
            c.selection_changed = true;
            c.needs_redraw = true;
        })
    );
    select(&mut ed, &[ai]);
    assert_eq!(
        ed.nudge_selected(1.0, 0.0, g(1)),
        changes(|c| {
            c.dirty_shapes.extend(ha.iter().copied());
            c.undo_state_changed = true;
            c.needs_redraw = true;
        })
    );
    assert_eq!(
        ed.nudge_selected(1.0, 0.0, g(1)),
        changes(|c| {
            c.dirty_shapes.extend(ha.iter().copied());
            c.needs_redraw = true;
        })
    );
    assert_eq!(ed.nudge_selected(0.0, 0.0, g(2)), changes(|_| {}));
    assert_eq!(ed.copy_shapes(), changes(|_| {}));
    assert_eq!(
        ed.bring_to_front(),
        changes(|c| {
            c.all_dirty = true;
            c.undo_state_changed = true;
            c.needs_redraw = true;
        })
    );
    assert_eq!(ed.bring_to_front(), changes(|_| {}));
    assert_eq!(
        ed.undo(),
        changes(|c| {
            c.all_dirty = true;
            c.undo_state_changed = true;
            c.needs_redraw = true;
        })
    );
    assert_eq!(
        ed.delete_selected(),
        changes(|c| {
            c.dirty_shapes.extend(ha.iter().copied());
            c.selection_changed = true;
            c.undo_state_changed = true;
            c.needs_redraw = true;
        })
    );
    assert_eq!(ed.delete_selected(), changes(|_| {}));
    assert_eq!(
        ed.undo(),
        changes(|c| {
            c.all_dirty = true;
            c.undo_state_changed = true;
            c.needs_redraw = true;
        })
    );
    assert_eq!(
        ed.apply_style(StylePatch::Swatch(Swatch::blue()), g(3)),
        changes(|_| {}),
        "nothing selected: only the next shape's style changes"
    );
    select(&mut ed, &[bi]);
    assert!(ed.duplicate_selected().selection_changed);
    let v = Viewport {
        scale: 2.0,
        ..*ed.viewport()
    };
    assert_eq!(ed.set_viewport(v), changes(|c| c.needs_redraw = true));
    assert_eq!(ed.set_viewport(v), changes(|_| {}));
}
