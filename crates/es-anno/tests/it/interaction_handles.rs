use crate::common;

use common::*;
use es_anno::{Editor, Handle, PointerInfo, Viewport};
use es_geom::{PVec, Point};
use es_model::{AnnoShapeId, ShapeKind, TextProps};
use std::f64::consts::{FRAC_PI_2, PI};

fn shift(p: PointerInfo) -> PointerInfo {
    PointerInfo { shift: true, ..p }
}

fn alt(p: PointerInfo) -> PointerInfo {
    PointerInfo { alt: true, ..p }
}

fn pv(p: PVec) -> PointerInfo {
    at(p.x, p.y)
}

/// x, y, w, h, rotation.
fn frame(ed: &Editor, id: AnnoShapeId) -> (f64, f64, f64, f64, f64) {
    let s = ed.doc().shape(id).expect("shape in document");
    let ShapeKind::Geo(p) = &s.kind else {
        panic!("not a box: {:?}", s.kind);
    };
    (s.x, s.y, p.w, p.h, s.rotation)
}

fn text_props(ed: &Editor, id: AnnoShapeId) -> TextProps {
    match &ed.doc().shape(id).expect("shape in document").kind {
        ShapeKind::Text(p) => p.clone(),
        other => panic!("not text: {other:?}"),
    }
}

fn close(a: (f64, f64, f64, f64, f64), b: (f64, f64, f64, f64, f64)) -> bool {
    let d = [a.0 - b.0, a.1 - b.1, a.2 - b.2, a.3 - b.3, a.4 - b.4];
    d.iter().all(|v| v.abs() < 1e-6)
}

fn selected_rect(x: f64, y: f64, w: f64, h: f64) -> (Editor, AnnoShapeId) {
    let r = rect(x, y, w, h);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    select(&mut ed, &[id]);
    (ed, id)
}

#[test]
fn the_frame_has_eight_resize_handles_then_four_rotate_handles_outside_the_corners() {
    let (ed, _) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let expect = [
        ((100.0, 100.0), Handle::TopLeft),
        ((200.0, 100.0), Handle::Top),
        ((300.0, 100.0), Handle::TopRight),
        ((300.0, 150.0), Handle::Right),
        ((300.0, 200.0), Handle::BottomRight),
        ((200.0, 200.0), Handle::Bottom),
        ((100.0, 200.0), Handle::BottomLeft),
        ((100.0, 150.0), Handle::Left),
    ];
    for ((x, y), h) in expect {
        assert_eq!(ed.handle_at(Point::new(x, y)), Some(h));
    }
    let out = PVec::new(100.0, -50.0).uni();
    let rotate = PVec::add(PVec::new(300.0, 100.0), PVec::mul(out, 14.0));
    assert_eq!(
        ed.handle_at(Point::new(rotate.x, rotate.y)),
        Some(Handle::Rotate { corner: 1 })
    );
    assert_eq!(ed.handle_at(Point::new(200.0, 150.0)), None);
}

#[test]
fn handles_are_nine_screen_pixels_wide_at_every_zoom() {
    let (mut ed, _) = selected_rect(100.0, 100.0, 100.0, 100.0);
    assert_eq!(
        ed.handle_at(Point::new(108.9, 100.0)),
        Some(Handle::TopLeft)
    );
    assert_eq!(ed.handle_at(Point::new(109.5, 100.0)), None);
    ed.set_viewport(Viewport {
        scale: 2.0,
        ..*ed.viewport()
    });
    assert_eq!(
        ed.handle_at(Point::new(208.9, 200.0)),
        Some(Handle::TopLeft)
    );
    assert_eq!(ed.handle_at(Point::new(209.5, 200.0)), None);
    let d = 14.0 / 2f64.sqrt();
    assert_eq!(
        ed.handle_at(Point::new(200.0 - d, 200.0 - d)),
        Some(Handle::Rotate { corner: 0 }),
        "the rotate zone sits 14 screen pixels out, not 14 page units"
    );
}

#[test]
fn there_are_no_handles_without_a_selection_or_outside_the_select_tool() {
    let r = rect(100.0, 100.0, 100.0, 100.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    assert_eq!(ed.handle_at(Point::new(100.0, 100.0)), None);
    select(&mut ed, &[id]);
    ed.select_tool(es_model::Tool::Rectangle);
    assert_eq!(ed.handle_at(Point::new(100.0, 100.0)), None);
}

#[test]
fn a_corner_scales_both_sides_about_the_opposite_corner_as_one_undo_step() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(300.0, 200.0), at(400.0, 250.0));
    assert!(close(frame(&ed, id), (100.0, 100.0, 300.0, 150.0, 0.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn shift_on_a_corner_keeps_the_aspect_ratio_from_the_larger_scale() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(300.0, 200.0), shift(at(400.0, 210.0)));
    assert!(close(frame(&ed, id), (100.0, 100.0, 300.0, 150.0, 0.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn alt_scales_about_the_frame_centre() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(300.0, 200.0), alt(at(350.0, 225.0)));
    assert!(close(frame(&ed, id), (50.0, 75.0, 300.0, 150.0, 0.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_side_handle_scales_only_its_own_axis() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(300.0, 150.0), at(350.0, 400.0));
    assert!(close(frame(&ed, id), (100.0, 100.0, 250.0, 100.0, 0.0)));
    one_undo_entry(&mut ed, &before);
    drag(&mut ed, at(225.0, 100.0), shift(at(500.0, 50.0)));
    assert!(
        close(frame(&ed, id), (100.0, 50.0, 250.0, 150.0, 0.0)),
        "shift keeps no aspect on a side"
    );
}

#[test]
fn dragging_a_side_past_the_opposite_one_flips_the_box_onto_the_pointer_side() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    drag(&mut ed, at(300.0, 150.0), at(50.0, 150.0));
    assert!(close(frame(&ed, id), (50.0, 100.0, 50.0, 100.0, 0.0)));
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    drag(&mut ed, at(300.0, 150.0), at(100.0, 150.0));
    assert!(
        close(frame(&ed, id), (100.0, 100.0, 2.0, 100.0, 0.0)),
        "the scale stops at 1 % instead of reaching zero"
    );
}

#[test]
fn a_rotated_shape_resizes_along_its_own_axes() {
    let mut r = rect(100.0, 100.0, 200.0, 100.0);
    r.rotation = FRAC_PI_2;
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    select(&mut ed, &[id]);
    assert_eq!(ed.handle_at(Point::new(50.0, 300.0)), Some(Handle::Right));
    drag(&mut ed, at(50.0, 300.0), at(50.0, 400.0));
    assert!(close(
        frame(&ed, id),
        (100.0, 100.0, 300.0, 100.0, FRAC_PI_2)
    ));
}

#[test]
fn several_shapes_scale_together_inside_their_shared_frame() {
    let a = rect(0.0, 0.0, 100.0, 100.0);
    let b = rect(200.0, 0.0, 100.0, 100.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai, bi]);
    let before = ed.doc().snapshot();
    drag(&mut ed, at(300.0, 100.0), at(600.0, 200.0));
    assert!(close(frame(&ed, ai), (0.0, 0.0, 200.0, 200.0, 0.0)));
    assert!(close(frame(&ed, bi), (400.0, 0.0, 200.0, 200.0, 0.0)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn a_text_side_handle_sets_a_wrap_width_and_a_corner_scales_the_type() {
    let t = text(100.0, 100.0, "hello");
    let id = t.id;
    let mut ed = editor(vec![t], vec![]);
    select(&mut ed, &[id]);
    let before = text_props(&ed, id);
    let size = ed.doc().measure_text(&before);
    let right = Point::new(100.0 + size.width, 100.0 + size.height / 2.0);
    let wider = Point::new(right.x + size.width, right.y);
    drag(&mut ed, at(right.x, right.y), at(wider.x, wider.y));
    let after = text_props(&ed, id);
    assert!(!after.auto_size, "a set width wraps");
    assert!((after.w - size.width * 2.0).abs() < 1e-6);
    assert_eq!(after.font_size, before.font_size);

    drag(
        &mut ed,
        at(100.0 + size.width, 100.0 + size.height),
        at(100.0 + size.width * 1.5, 100.0 + size.height * 1.5),
    );
    let scaled = text_props(&ed, id);
    assert!((scaled.font_size - before.font_size * 1.5).abs() < 1e-6);
    assert!((scaled.w - after.w * 1.5).abs() < 1e-6);
}

#[test]
fn rotating_turns_the_selection_about_its_centre_and_shift_snaps_the_turn() {
    let (mut ed, id) = selected_rect(100.0, 100.0, 200.0, 100.0);
    let center = PVec::new(200.0, 150.0);
    let out = PVec::new(100.0, -50.0).uni();
    let grip = PVec::add(PVec::new(300.0, 100.0), PVec::mul(out, 14.0));
    let turned = PVec::rot_with(grip, center, 20f64.to_radians());
    let before = ed.doc().snapshot();
    drag(&mut ed, pv(grip), pv(turned));
    let at20 = PVec::rot_with(PVec::new(100.0, 100.0), center, 20f64.to_radians());
    assert!(close(
        frame(&ed, id),
        (at20.x, at20.y, 200.0, 100.0, 20f64.to_radians())
    ));
    one_undo_entry(&mut ed, &before);

    ed.undo();
    drag(&mut ed, pv(grip), shift(pv(turned)));
    let step = PI / 12.0;
    let at15 = PVec::rot_with(PVec::new(100.0, 100.0), center, step);
    assert!(close(frame(&ed, id), (at15.x, at15.y, 200.0, 100.0, step)));
    one_undo_entry(&mut ed, &before);
}

#[test]
fn several_shapes_rotate_about_their_shared_centre() {
    let a = rect(0.0, 0.0, 100.0, 100.0);
    let b = rect(200.0, 0.0, 100.0, 100.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai, bi]);
    let center = PVec::new(150.0, 50.0);
    let out = PVec::mul(PVec::new(150.0, 50.0).uni(), 14.0);
    let grip = PVec::add(PVec::new(300.0, 100.0), out);
    drag(
        &mut ed,
        pv(grip),
        shift(pv(PVec::rot_with(grip, center, FRAC_PI_2))),
    );
    let pa = PVec::rot_with(PVec::new(0.0, 0.0), center, FRAC_PI_2);
    let pb = PVec::rot_with(PVec::new(200.0, 0.0), center, FRAC_PI_2);
    assert!(close(frame(&ed, ai), (pa.x, pa.y, 100.0, 100.0, FRAC_PI_2)));
    assert!(close(frame(&ed, bi), (pb.x, pb.y, 100.0, 100.0, FRAC_PI_2)));
}

#[test]
fn a_press_on_a_handle_without_moving_leaves_no_undo_entry() {
    let (mut ed, _) = selected_rect(100.0, 100.0, 200.0, 100.0);
    ed.pointer_down(at(300.0, 200.0));
    ed.pointer_up(at(300.0, 200.0));
    assert!(!ed.can_undo());
    assert!(ed.is_idle());
}

#[test]
fn a_handle_wins_over_a_shape_under_it() {
    let r = rect(100.0, 100.0, 200.0, 100.0);
    let other = filled_rect(280.0, 180.0, 100.0, 100.0);
    let (ri, oi) = (r.id, other.id);
    let mut ed = editor(vec![r, other], vec![]);
    select(&mut ed, &[ri]);
    drag(&mut ed, at(300.0, 200.0), at(400.0, 250.0));
    assert!(close(frame(&ed, ri), (100.0, 100.0, 300.0, 150.0, 0.0)));
    assert!(close(frame(&ed, oi), (280.0, 180.0, 100.0, 100.0, 0.0)));
}
