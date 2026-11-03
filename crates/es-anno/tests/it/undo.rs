use crate::common;

use common::*;
use es_anno::editor::UNDO_LIMIT;
use es_anno::{Editor, Snapshot, StylePatch};
use es_geom::Point;
use es_model::{AnnoShapeId, ShapeKind, Swatch, TextAlign, Tool};

fn x_of(ed: &Editor, id: AnnoShapeId) -> f64 {
    ed.doc().shape(id).expect("shape in document").x
}

fn text_of(ed: &Editor, id: AnnoShapeId) -> Option<String> {
    match &ed.doc().shape(id)?.kind {
        ShapeKind::Text(p) => Some(p.text.clone()),
        _ => None,
    }
}

/// Undo returns to exactly the state before the command and redo to exactly the state after.
fn round_trips(ed: &mut Editor, command: impl FnOnce(&mut Editor)) {
    let before: Snapshot = ed.doc().snapshot();
    command(ed);
    let after = ed.doc().snapshot();
    assert_ne!(before, after, "the command must change the document");
    ed.undo();
    assert_eq!(ed.doc().snapshot(), before);
    ed.redo();
    assert_eq!(ed.doc().snapshot(), after);
    ed.undo();
    assert_eq!(ed.doc().snapshot(), before);
}

fn scene() -> (Editor, AnnoShapeId, AnnoShapeId) {
    let a = rect(0.0, 0.0, 50.0, 50.0);
    let b = rect(100.0, 0.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    (editor(vec![a, b], vec![]), ai, bi)
}

#[test]
fn every_command_round_trips_through_undo_and_redo() {
    let (mut ed, a, b) = scene();
    select(&mut ed, &[a]);
    round_trips(&mut ed, |ed| {
        ed.delete_selected();
    });
    select(&mut ed, &[a]);
    round_trips(&mut ed, |ed| {
        ed.nudge_selected(5.0, 0.0, g(1));
    });
    select(&mut ed, &[a]);
    round_trips(&mut ed, |ed| {
        ed.duplicate_selected();
    });
    select(&mut ed, &[a]);
    round_trips(&mut ed, |ed| {
        ed.apply_style(StylePatch::Swatch(Swatch::blue()), g(2));
    });
    select(&mut ed, &[a]);
    round_trips(&mut ed, |ed| {
        ed.bring_to_front();
    });
    select(&mut ed, &[b]);
    round_trips(&mut ed, |ed| {
        ed.copy_shapes();
        ed.paste_shapes(None);
    });
    round_trips(&mut ed, |ed| {
        ed.create_text(Point::new(500.0, 500.0));
        ed.update_text("hi");
        ed.stop_text();
    });
}

#[test]
fn gestures_that_change_nothing_leave_no_undo_entry() {
    let (mut ed, a, b) = scene();
    select(&mut ed, &[a]);
    ed.nudge_selected(0.0, 0.0, g(1));
    ed.apply_style(StylePatch::Swatch(Swatch::red()), g(2));
    ed.apply_style(StylePatch::RedactionDensity(0.3), g(3));
    select(&mut ed, &[b]);
    ed.bring_to_front();
    ed.bring_forward();
    select(&mut ed, &[a]);
    ed.send_to_back();
    ed.send_backward();
    ed.create_text(Point::new(10.0, 10.0));
    ed.stop_text();
    ed.select_all();
    ed.copy_shapes();
    assert!(!ed.can_undo());
    assert_eq!(ed.doc().shapes().len(), 2);
}

#[test]
fn a_no_op_gesture_keeps_the_redo_stack() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    ed.nudge_selected(5.0, 0.0, g(1));
    ed.undo();
    assert!(ed.can_redo());
    ed.nudge_selected(0.0, 0.0, g(2));
    ed.apply_style(StylePatch::Swatch(Swatch::red()), g(3));
    assert!(ed.can_redo());
}

#[test]
fn one_slider_drag_makes_one_undo_entry() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    for step in 1..=10 {
        ed.apply_style(StylePatch::StrokeWidth(f64::from(step)), g(7));
    }
    assert_eq!(ed.doc().shape(a).unwrap().stroke_width(), 20.0);
    ed.apply_style(StylePatch::StrokeWidth(12.0), g(8));
    ed.undo();
    assert_eq!(ed.doc().shape(a).unwrap().stroke_width(), 20.0);
    ed.undo();
    assert_eq!(ed.doc().shape(a).unwrap().stroke_width(), 8.0);
    assert!(!ed.can_undo());
}

#[test]
fn one_key_hold_makes_one_undo_entry_and_the_next_press_another() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    for _ in 0..5 {
        ed.nudge_selected(1.0, 0.0, g(1));
    }
    ed.nudge_selected(10.0, 0.0, g(2));
    assert_eq!(x_of(&ed, a), 15.0);
    ed.undo();
    assert_eq!(x_of(&ed, a), 5.0);
    ed.undo();
    assert_eq!(x_of(&ed, a), 0.0);
}

#[test]
fn the_stack_keeps_the_newest_entries_up_to_the_limit() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    for i in 0..=UNDO_LIMIT as u64 {
        ed.nudge_selected(1.0, 0.0, g(i));
    }
    let mut undone = 0;
    while ed.can_undo() {
        ed.undo();
        undone += 1;
    }
    assert_eq!(undone, UNDO_LIMIT);
    assert_eq!(x_of(&ed, a), 1.0, "only the oldest entry is dropped");
}

#[test]
fn a_new_edit_clears_redo() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    ed.nudge_selected(1.0, 0.0, g(1));
    ed.undo();
    ed.nudge_selected(0.0, 1.0, g(2));
    assert!(!ed.can_redo());
}

#[test]
fn undo_forgets_the_selection_of_shapes_it_removes() {
    let (mut ed, a, _) = scene();
    select(&mut ed, &[a]);
    ed.duplicate_selected();
    assert_eq!(ed.selected_ids().len(), 1);
    let ch = ed.undo();
    assert!(ch.selection_changed);
    assert!(ed.selected_ids().is_empty());
}

#[test]
fn a_typing_session_is_one_undo_entry() {
    let (mut ed, _, _) = scene();
    ed.create_text(Point::new(300.0, 300.0));
    let id = ed.editing_text_id().unwrap();
    for s in ["h", "he", "hel", "hello"] {
        ed.update_text(s);
    }
    ed.stop_text();
    assert_eq!(text_of(&ed, id).as_deref(), Some("hello"));
    ed.undo();
    assert!(ed.doc().shape(id).is_none());
    assert!(!ed.can_undo());
}

#[test]
fn editing_existing_text_is_one_entry_and_emptying_it_deletes_it() {
    let t = text(0.0, 0.0, "draft");
    let id = t.id;
    let mut ed = editor(vec![t], vec![]);
    ed.begin_text(id);
    ed.update_text("dra");
    ed.update_text("");
    ed.stop_text();
    assert!(ed.doc().shape(id).is_none());
    ed.undo();
    assert_eq!(text_of(&ed, id).as_deref(), Some("draft"));
    assert!(!ed.can_undo());
}

#[test]
fn an_abandoned_new_text_leaves_no_trace() {
    let (mut ed, _, _) = scene();
    ed.create_text(Point::new(300.0, 300.0));
    ed.update_text("x");
    ed.update_text("  ");
    let ch = ed.stop_text();
    assert!(ch.undo_state_changed);
    assert_eq!(ed.doc().shapes().len(), 2);
    assert!(!ed.can_undo());
}

#[test]
fn undo_while_typing_removes_the_new_text() {
    let (mut ed, _, _) = scene();
    ed.create_text(Point::new(300.0, 300.0));
    ed.update_text("abc");
    ed.undo();
    assert_eq!(ed.editing_text_id(), None);
    assert_eq!(ed.doc().shapes().len(), 2);
}

#[test]
fn style_changes_while_typing_join_the_editing_session() {
    let (mut ed, _, _) = scene();
    ed.create_text(Point::new(300.0, 300.0));
    ed.update_text("a");
    ed.apply_style(StylePatch::Bold(false), g(1));
    ed.apply_style(StylePatch::FontSize(60.0), g(2));
    ed.update_text("ab");
    ed.stop_text();
    ed.undo();
    assert_eq!(ed.doc().shapes().len(), 2);
    assert!(!ed.can_undo());
}

#[test]
fn centred_and_end_aligned_text_keep_their_anchor_while_typing() {
    type Anchor = fn(f64, f64) -> f64;
    let cases: [(TextAlign, Anchor); 3] = [
        (TextAlign::Middle, |x, w| x + w / 2.0),
        (TextAlign::End, |x, w| x + w),
        (TextAlign::Start, |x, _| x),
    ];
    for (align, anchor) in cases {
        let (mut ed, _, _) = scene();
        ed.apply_style(StylePatch::Align(align), g(1));
        ed.create_text(Point::new(400.0, 300.0));
        let id = ed.editing_text_id().unwrap();
        for s in ["a", "abcdefgh", "abc"] {
            ed.update_text(s);
            let w = ed.doc().geometry(id).unwrap().bounds().w;
            assert!((anchor(x_of(&ed, id), w) - 400.0).abs() < 1e-9, "{align:?}");
        }
    }
}

fn text_props(ed: &Editor, id: AnnoShapeId) -> es_model::TextProps {
    match &ed.doc().shape(id).expect("shape in document").kind {
        ShapeKind::Text(p) => p.clone(),
        _ => panic!("text"),
    }
}

#[test]
fn text_fill_and_border_setters_undo_and_redo() {
    let t = text(10.0, 10.0, "abc");
    let id = t.id;
    let mut ed = editor(vec![t], vec![]);
    select(&mut ed, &[id]);
    round_trips(&mut ed, |ed| {
        ed.set_text_fill(Some(Swatch::blue()), g(1));
    });
    assert!(text_props(&ed, id).fill.is_none());
    round_trips(&mut ed, |ed| {
        let b = es_model::TextBorder::default();
        ed.set_text_border(Some(b), g(2));
    });
    assert!(text_props(&ed, id).border.is_none());
    ed.set_text_fill(Some(Swatch::blue()), g(3));
    ed.set_text_fill(None, g(4));
    assert!(text_props(&ed, id).fill.is_none());
}

#[test]
fn text_box_setters_ignore_a_selection_that_is_not_one_text_and_a_bad_width() {
    let r = rect(0.0, 0.0, 50.0, 50.0);
    let t = text(100.0, 0.0, "abc");
    let (rid, tid) = (r.id, t.id);
    let mut ed = editor(vec![r, t], vec![]);
    select(&mut ed, &[rid]);
    let before = ed.doc().snapshot();
    ed.set_text_fill(Some(Swatch::blue()), g(1));
    assert_eq!(ed.doc().snapshot(), before);
    select(&mut ed, &[tid]);
    let bad = es_model::TextBorder {
        width: f64::NAN,
        ..es_model::TextBorder::default()
    };
    ed.set_text_border(Some(bad), g(2));
    assert_eq!(ed.doc().snapshot(), before);
}

#[test]
fn text_box_changes_made_while_typing_join_the_typing_undo_entry() {
    let (mut ed, _, _) = scene();
    ed.select_tool(Tool::Text);
    ed.create_text(Point::new(300.0, 300.0));
    let id = ed.editing_text_id().expect("editing");
    ed.update_text("hi");
    ed.set_text_fill(Some(Swatch::blue()), g(9));
    assert!(text_props(&ed, id).fill.is_some());
    ed.stop_text();
    ed.undo();
    assert!(ed.doc().shape(id).is_none());
}

#[test]
fn switching_tools_commits_the_text_being_edited() {
    let (mut ed, _, _) = scene();
    ed.select_tool(Tool::Text);
    ed.create_text(Point::new(300.0, 300.0));
    ed.update_text("kept");
    ed.select_tool(Tool::Rectangle);
    assert_eq!(ed.editing_text_id(), None);
    assert_eq!(ed.doc().shapes().len(), 3);
    assert!(ed.can_undo());
}
