use crate::common;

use common::*;
use es_anno::{CursorKind, Editor, PointerInfo};
use es_geom::Point;
use es_model::{AnnoShapeId, ShapeKind, Tool};

fn click(ed: &mut Editor, p: PointerInfo) {
    ed.pointer_down(p);
    ed.pointer_up(p);
}

fn text_of(ed: &Editor, id: AnnoShapeId) -> Option<String> {
    match &ed.doc().shape(id)?.kind {
        ShapeKind::Text(p) => Some(p.text.clone()),
        _ => None,
    }
}

fn text_tool() -> Editor {
    let mut ed = editor(vec![], vec![]);
    ed.select_tool(Tool::Text);
    ed
}

#[test]
fn a_text_tool_click_creates_text_centred_on_the_first_line_and_starts_typing() {
    let mut ed = text_tool();
    let before = ed.doc().snapshot();
    click(&mut ed, at(300.0, 200.0));
    let id = ed.editing_text_id().expect("typing into the new text");
    let s = ed.doc().shape(id).expect("text in document");
    let size = es_model::shape::DEFAULT_TEXT_SIZE;
    assert_eq!((s.x, s.y), (300.0, 200.0 - size * 1.35 / 2.0));
    assert_eq!(
        ed.selected_ids().iter().copied().collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(ed.cursor_for(Point::new(305.0, 200.0)), CursorKind::IBeam);

    ed.update_text("Hello");
    ed.update_text("Hello there");
    ed.stop_text();
    assert_eq!(text_of(&ed, id).as_deref(), Some("Hello there"));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn clicking_elsewhere_commits_the_text_and_starts_new_text_there() {
    let mut ed = text_tool();
    click(&mut ed, at(100.0, 100.0));
    let first = ed.editing_text_id().expect("editing");
    ed.update_text("one");
    click(&mut ed, at(600.0, 600.0));
    let second = ed.editing_text_id().expect("editing again");
    assert_ne!(first, second);
    assert_eq!(text_of(&ed, first).as_deref(), Some("one"));

    click(&mut ed, at(900.0, 100.0));
    assert!(
        ed.doc().shape(second).is_none(),
        "text left blank is removed"
    );
    assert_eq!(ed.doc().shapes().len(), 2);
}

#[test]
fn a_text_tool_click_on_existing_text_edits_it_instead_of_making_more() {
    let t = text(200.0, 200.0, "hello world");
    let ti = t.id;
    let mut ed = editor(vec![t], vec![]);
    ed.select_tool(Tool::Text);
    assert_eq!(ed.cursor_for(Point::new(240.0, 212.0)), CursorKind::IBeam);
    assert_eq!(
        ed.cursor_for(Point::new(600.0, 600.0)),
        CursorKind::IBeam,
        "the text tool shows the I-beam where a click starts new text"
    );
    click(&mut ed, at(240.0, 212.0));
    assert_eq!(ed.editing_text_id(), Some(ti));
    assert_eq!(ed.doc().shapes().len(), 1);
}

#[test]
fn a_press_on_the_text_being_typed_is_left_to_the_text_field() {
    let mut ed = text_tool();
    click(&mut ed, at(300.0, 200.0));
    let id = ed.editing_text_id().expect("editing");
    ed.update_text("abc");
    let ch = ed.pointer_down(at(305.0, 200.0));
    ed.pointer_up(at(305.0, 200.0));
    assert_eq!(ch, es_anno::Changes::default());
    assert_eq!(ed.editing_text_id(), Some(id));
}

#[test]
fn a_double_click_edits_text_in_the_select_tool_and_a_single_click_does_not() {
    let t = text(200.0, 200.0, "hello world");
    let ti = t.id;
    let mut ed = editor(vec![t], vec![]);
    click(&mut ed, at(240.0, 212.0));
    assert_eq!(ed.editing_text_id(), None);
    click(
        &mut ed,
        PointerInfo {
            click_count: 2,
            ..at(240.0, 212.0)
        },
    );
    assert_eq!(ed.editing_text_id(), Some(ti));
    assert_eq!(ed.cursor_for(Point::new(240.0, 212.0)), CursorKind::IBeam);

    let before = ed.doc().snapshot();
    ed.update_text("hello you");
    click(&mut ed, at(900.0, 900.0));
    assert_eq!(ed.editing_text_id(), None, "a press elsewhere commits");
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_double_click_on_a_shape_that_is_not_text_only_selects() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    click(
        &mut ed,
        PointerInfo {
            click_count: 2,
            ..at(10.0, 10.0)
        },
    );
    assert_eq!(ed.editing_text_id(), None);
    assert!(ed.selected_ids().contains(&ai));
}
