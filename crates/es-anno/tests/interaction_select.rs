mod common;

use common::*;
use es_anno::{CursorKind, Editor, PointerInfo};
use es_geom::Point;
use es_model::{AnnoShapeId, Tool};
use std::collections::BTreeSet;

fn shift(p: PointerInfo) -> PointerInfo {
    PointerInfo { shift: true, ..p }
}

fn click(ed: &mut Editor, p: PointerInfo) {
    ed.pointer_down(p);
    ed.pointer_up(p);
}

fn selected(ed: &Editor) -> BTreeSet<AnnoShapeId> {
    ed.selected_ids().clone()
}

fn set(ids: &[AnnoShapeId]) -> BTreeSet<AnnoShapeId> {
    ids.iter().copied().collect()
}

fn pos(ed: &Editor, id: AnnoShapeId) -> (f64, f64) {
    let s = ed.doc().shape(id).expect("shape in document");
    (s.x, s.y)
}

#[test]
fn a_click_selects_the_topmost_shape_under_the_pointer_and_empty_space_clears() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let b = filled_rect(20.0, 20.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    click(&mut ed, at(30.0, 30.0));
    assert_eq!(selected(&ed), set(&[bi]));
    click(&mut ed, at(5.0, 5.0));
    assert_eq!(selected(&ed), set(&[ai]));
    click(&mut ed, at(500.0, 500.0));
    assert!(selected(&ed).is_empty());
    assert!(!ed.can_undo(), "selecting is not an edit");
}

#[test]
fn a_hollow_shape_is_grabbed_within_the_hit_margin_of_its_outline() {
    let a = rect(100.0, 100.0, 200.0, 200.0);
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    click(&mut ed, at(93.0, 150.0));
    assert_eq!(selected(&ed), set(&[ai]));
    click(&mut ed, at(500.0, 500.0));
    click(&mut ed, at(90.0, 150.0));
    assert!(selected(&ed).is_empty(), "10 units off the outline misses");
}

#[test]
fn shift_click_toggles_a_shape_in_and_out_of_the_selection() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let b = filled_rect(100.0, 0.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    click(&mut ed, at(10.0, 10.0));
    click(&mut ed, shift(at(110.0, 10.0)));
    assert_eq!(selected(&ed), set(&[ai, bi]));
    click(&mut ed, shift(at(10.0, 10.0)));
    assert_eq!(selected(&ed), set(&[bi]));
    click(&mut ed, shift(at(500.0, 500.0)));
    assert_eq!(
        selected(&ed),
        set(&[bi]),
        "shift on empty space keeps the selection"
    );
}

#[test]
fn a_click_on_a_selected_shape_keeps_the_multi_selection() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let b = filled_rect(100.0, 0.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai, bi]);
    click(&mut ed, at(10.0, 10.0));
    assert_eq!(selected(&ed), set(&[ai, bi]));
}

#[test]
fn dragging_a_selected_shape_moves_the_whole_selection_as_one_undo_step() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let b = filled_rect(100.0, 0.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai, bi]);
    let before = ed.doc().snapshot();
    ed.pointer_down(at(10.0, 10.0));
    for (x, y) in [(15.0, 12.0), (25.0, 18.0), (40.0, 30.0)] {
        ed.pointer_move(at(x, y));
        assert_eq!(ed.cursor_for(Point::new(x, y)), CursorKind::ClosedHand);
    }
    ed.pointer_up(at(40.0, 30.0));
    assert_eq!((pos(&ed, ai), pos(&ed, bi)), ((30.0, 20.0), (130.0, 20.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn dragging_an_unselected_shape_selects_it_and_moves_only_it() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let b = filled_rect(100.0, 0.0, 50.0, 50.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai]);
    drag(&mut ed, at(110.0, 10.0), at(120.0, 60.0));
    assert_eq!(selected(&ed), set(&[bi]));
    assert_eq!((pos(&ed, ai), pos(&ed, bi)), ((0.0, 0.0), (110.0, 50.0)));
}

#[test]
fn shift_locks_a_move_to_the_axis_it_moved_furthest_along() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    select(&mut ed, &[ai]);
    drag(&mut ed, at(10.0, 10.0), shift(at(40.0, 20.0)));
    assert_eq!(pos(&ed, ai), (30.0, 0.0));
    drag(&mut ed, at(40.0, 10.0), shift(at(45.0, 60.0)));
    assert_eq!(pos(&ed, ai), (30.0, 50.0));
}

#[test]
fn a_press_on_a_shape_without_moving_leaves_no_undo_entry() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let mut ed = editor(vec![a], vec![]);
    click(&mut ed, at(10.0, 10.0));
    assert!(!ed.can_undo());
    assert!(ed.is_idle());
}

#[test]
fn the_inside_of_a_selected_hollow_shape_drags_it() {
    let a = rect(0.0, 0.0, 200.0, 200.0);
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    assert_eq!(ed.cursor_for(Point::new(100.0, 100.0)), CursorKind::Arrow);
    select(&mut ed, &[ai]);
    assert_eq!(
        ed.cursor_for(Point::new(100.0, 100.0)),
        CursorKind::OpenHand
    );
    drag(&mut ed, at(100.0, 100.0), at(130.0, 110.0));
    assert_eq!(pos(&ed, ai), (30.0, 10.0));
    assert_eq!(selected(&ed), set(&[ai]));
}

#[test]
fn a_lone_selected_arrow_is_not_dragged_from_its_empty_frame() {
    let a = arrow(0.0, 0.0, (0.0, 0.0), (200.0, 200.0));
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    select(&mut ed, &[ai]);
    assert_eq!(ed.cursor_for(Point::new(180.0, 20.0)), CursorKind::Arrow);
    drag(&mut ed, at(180.0, 20.0), at(190.0, 30.0));
    assert_eq!(pos(&ed, ai), (0.0, 0.0));
    assert!(selected(&ed).is_empty());
}

#[test]
fn a_marquee_selects_every_shape_it_touches_on_release() {
    let a = rect(0.0, 0.0, 50.0, 50.0);
    let b = rect(100.0, 0.0, 50.0, 50.0);
    let c = rect(300.0, 300.0, 50.0, 50.0);
    let (ai, bi, ci) = (a.id, b.id, c.id);
    let mut ed = editor(vec![a, b, c], vec![]);
    select(&mut ed, &[ci]);
    ed.pointer_down(at(-10.0, -10.0));
    assert!(
        selected(&ed).is_empty(),
        "a plain marquee starts by clearing"
    );
    ed.pointer_move(at(60.0, 20.0));
    ed.pointer_move(at(110.0, 20.0));
    assert!(
        selected(&ed).is_empty(),
        "nothing is selected until release"
    );
    assert_eq!(ed.cursor_for(Point::new(110.0, 20.0)), CursorKind::Arrow);
    let brush = ed.brush().expect("a marquee while brushing");
    assert_eq!(
        (brush.x0, brush.y0, brush.x1, brush.y1),
        (-10.0, -10.0, 110.0, 20.0)
    );
    ed.pointer_up(at(110.0, 20.0));
    assert_eq!(selected(&ed), set(&[ai, bi]));
    assert!(ed.brush().is_none());
    assert!(!ed.can_undo());
}

#[test]
fn a_marquee_inside_a_hollow_shape_touches_nothing() {
    let a = rect(0.0, 0.0, 400.0, 400.0);
    let mut ed = editor(vec![a], vec![]);
    drag(&mut ed, at(100.0, 100.0), at(200.0, 200.0));
    assert!(selected(&ed).is_empty());
}

#[test]
fn a_shift_marquee_adds_to_the_selection() {
    let a = rect(0.0, 0.0, 50.0, 50.0);
    let c = rect(300.0, 300.0, 50.0, 50.0);
    let (ai, ci) = (a.id, c.id);
    let mut ed = editor(vec![a, c], vec![]);
    select(&mut ed, &[ai]);
    drag(&mut ed, shift(at(280.0, 280.0)), shift(at(320.0, 320.0)));
    assert_eq!(selected(&ed), set(&[ai, ci]));
}

#[test]
fn select_shows_an_open_hand_over_shapes_and_an_arrow_elsewhere() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let t = text(200.0, 200.0, "hi");
    let ed = editor(vec![a, t], vec![]);
    assert_eq!(ed.cursor_for(Point::new(10.0, 10.0)), CursorKind::OpenHand);
    assert_eq!(
        ed.cursor_for(Point::new(210.0, 220.0)),
        CursorKind::OpenHand
    );
    assert_eq!(ed.cursor_for(Point::new(500.0, 500.0)), CursorKind::Arrow);
}

#[test]
fn hover_follows_the_viewport_from_screen_to_page() {
    let a = filled_rect(100.0, 100.0, 50.0, 50.0);
    let mut ed = editor(vec![a], vec![]);
    ed.set_viewport(es_anno::Viewport {
        origin: Point::new(20.0, 20.0),
        scale: 2.0,
        ..*ed.viewport()
    });
    assert_eq!(
        ed.cursor_for(Point::new(250.0, 250.0)),
        CursorKind::OpenHand
    );
    assert_eq!(ed.cursor_for(Point::new(120.0, 120.0)), CursorKind::Arrow);
}

#[test]
fn leaving_select_for_a_drawing_tool_stops_moving_shapes() {
    let a = filled_rect(0.0, 0.0, 50.0, 50.0);
    let ai = a.id;
    let mut ed = editor(vec![a], vec![]);
    ed.select_tool(Tool::Rectangle);
    drag(&mut ed, at(10.0, 10.0), at(110.0, 110.0));
    assert_eq!(pos(&ed, ai), (0.0, 0.0));
    assert_eq!(ed.doc().shapes().len(), 2);
}
