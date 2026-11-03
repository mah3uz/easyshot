use crate::common;

use common::*;
use es_anno::{Editor, PointerInfo, Viewport};
use es_geom::{BBox, PVec, Point, Size};
use es_model::{AnnoShapeId, ShapeKind, Tool};
use std::f64::consts::FRAC_PI_4;

const W: f64 = 1800.0;
const H: f64 = 1000.0;

fn armed(tool: Tool) -> Editor {
    let mut ed = editor(vec![], vec![]);
    ed.select_tool(tool);
    ed
}

fn bounds(ed: &Editor, id: AnnoShapeId) -> BBox {
    ed.doc().page_bounds(id).expect("shape in document")
}

fn inside(b: BBox, w: f64, h: f64) -> bool {
    let e = 1e-6;
    b.min_x() >= -e && b.min_y() >= -e && b.max_x() <= w + e && b.max_y() <= h + e
}

fn centre(ed: &Editor, id: AnnoShapeId) -> PointerInfo {
    let b = bounds(ed, id);
    at(b.mid_x(), b.mid_y())
}

#[test]
fn a_press_off_the_image_starts_nothing_with_any_creation_tool() {
    for tool in [
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Highlight,
        Tool::Blur,
        Tool::Line,
        Tool::Arrow,
        Tool::Freehand,
        Tool::NumberedCircle,
        Tool::Text,
    ] {
        for (x, y) in [
            (-5.0, 100.0),
            (100.0, -5.0),
            (W + 5.0, 100.0),
            (100.0, H + 5.0),
        ] {
            let mut ed = armed(tool);
            drag(&mut ed, at(x, y), at(x + 40.0, y + 40.0));
            assert!(ed.doc().shapes().is_empty(), "{tool:?} at ({x}, {y})");
        }
    }
}

#[test]
fn a_box_dragged_past_the_edge_stops_at_it() {
    let mut ed = armed(Tool::Rectangle);
    drag(&mut ed, at(1700.0, 900.0), at(2500.0, 1500.0));
    let b = bounds(&ed, ed.doc().shapes()[0].id);
    assert!((b.max_x() - W).abs() < 1e-6 && (b.max_y() - H).abs() < 1e-6);

    let mut ed = armed(Tool::Rectangle);
    drag(&mut ed, at(100.0, 100.0), at(-400.0, -400.0));
    let b = bounds(&ed, ed.doc().shapes()[0].id);
    assert!(inside(b, W, H) && b.min_x().abs() < 1e-6);
}

#[test]
fn a_square_centred_box_stops_when_either_side_meets_the_edge() {
    let mut ed = armed(Tool::Rectangle);
    let keys = |p: PointerInfo| PointerInfo {
        alt: true,
        shift: true,
        ..p
    };
    drag(&mut ed, keys(at(1700.0, 500.0)), keys(at(2200.0, 500.0)));
    let b = bounds(&ed, ed.doc().shapes()[0].id);
    assert!(inside(b, W, H));
    assert!((b.w - b.h).abs() < 1e-6, "still square");
}

#[test]
fn an_arrow_and_a_stroke_dragged_past_the_edge_end_on_it() {
    let mut ed = armed(Tool::Arrow);
    drag(&mut ed, at(100.0, 100.0), at(3000.0, 100.0));
    let b = bounds(&ed, ed.doc().shapes()[0].id);
    assert!((b.max_x() - W).abs() < 1e-6);

    let mut ed = armed(Tool::Freehand);
    drag(&mut ed, at(100.0, 100.0), at(100.0, 4000.0));
    let b = bounds(&ed, ed.doc().shapes()[0].id);
    assert!(inside(b, W, H));
}

#[test]
fn a_click_creation_near_the_edge_is_nudged_inside() {
    for (tool, x, y) in [
        (Tool::Rectangle, W - 2.0, H - 2.0),
        (Tool::NumberedCircle, 1.0, 1.0),
        (Tool::Text, W - 1.0, H - 1.0),
    ] {
        let mut ed = armed(tool);
        drag(&mut ed, at(x, y), at(x, y));
        let b = bounds(&ed, ed.doc().shapes()[0].id);
        assert!(inside(b, W, H), "{tool:?}");
    }
}

#[test]
fn a_move_past_the_edge_stops_at_it() {
    let r = filled_rect(100.0, 100.0, 200.0, 100.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    drag(&mut ed, at(150.0, 150.0), at(-900.0, -900.0));
    let b = bounds(&ed, id);
    assert!(b.min_x().abs() < 1e-6 && b.min_y().abs() < 1e-6);

    let from = centre(&ed, id);
    drag(&mut ed, from, at(9000.0, 9000.0));
    let b = bounds(&ed, id);
    assert!((b.max_x() - W).abs() < 1e-6 && (b.max_y() - H).abs() < 1e-6);
}

#[test]
fn a_selection_moves_as_one_and_stops_when_its_outermost_shape_meets_the_edge() {
    let a = rect(100.0, 100.0, 100.0, 100.0);
    let b = rect(1000.0, 100.0, 100.0, 100.0);
    let (ai, bi) = (a.id, b.id);
    let mut ed = editor(vec![a, b], vec![]);
    select(&mut ed, &[ai, bi]);
    drag(&mut ed, at(150.0, 150.0), at(5000.0, 150.0));
    assert!((bounds(&ed, bi).max_x() - W).abs() < 1e-6);
    assert!((bounds(&ed, bi).min_x() - bounds(&ed, ai).min_x() - 900.0).abs() < 1e-6);
}

#[test]
fn a_resize_past_the_edge_stops_at_it() {
    let r = rect(1600.0, 800.0, 100.0, 100.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    select(&mut ed, &[id]);
    drag(&mut ed, at(1700.0, 900.0), at(5000.0, 5000.0));
    let b = bounds(&ed, id);
    assert!(b.w > 100.0, "it still grew");
    assert!(
        (b.max_x() - W).abs() < 1e-3 && (b.max_y() - H).abs() < 1e-3,
        "{b:?}"
    );
}

#[test]
fn a_rotated_shape_is_held_by_its_rotated_extent() {
    let mut r = filled_rect(300.0, 300.0, 200.0, 40.0);
    r.rotation = FRAC_PI_4;
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);

    let from = centre(&ed, id);
    drag(&mut ed, from, at(-9000.0, -9000.0));
    let b = bounds(&ed, id);
    assert!(inside(b, W, H) && b.min_x().abs() < 1e-6);

    let from = centre(&ed, id);
    drag(&mut ed, from, at(9000.0, 9000.0));
    let b = bounds(&ed, id);
    assert!(inside(b, W, H) && (b.max_y() - H).abs() < 1e-6);
}

#[test]
fn resizing_or_turning_a_rotated_shape_keeps_it_inside() {
    let mut r = rect(1500.0, 700.0, 200.0, 40.0);
    r.rotation = FRAC_PI_4;
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    select(&mut ed, &[id]);
    let grip = ed
        .selection_bounds()
        .expect("a selection")
        .page_point(PVec::new(1.0, 1.0));
    drag(
        &mut ed,
        at(grip.x, grip.y),
        at(grip.x + 4000.0, grip.y + 4000.0),
    );
    assert!(inside(bounds(&ed, id), W, H));

    let r = rect(1500.0, 800.0, 150.0, 150.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    select(&mut ed, &[id]);
    let b = ed.selection_bounds().expect("a selection");
    let center = b.page_center();
    let corner = b.page_point(PVec::new(1.0, 1.0));
    let rot = PVec::add(corner, PVec::mul(PVec::sub(corner, center).uni(), 14.0));
    let turned = PVec::rot_with(rot, center, FRAC_PI_4);
    drag(&mut ed, at(rot.x, rot.y), at(turned.x, turned.y));
    assert!(inside(bounds(&ed, id), W, H));
    assert!(ed.doc().shape(id).expect("shape").rotation > 0.0);
}

#[test]
fn the_image_is_the_cropped_image_once_the_viewport_says_so() {
    let r = filled_rect(100.0, 100.0, 100.0, 100.0);
    let id = r.id;
    let mut ed = editor(vec![r], vec![]);
    ed.set_viewport(Viewport {
        origin: Point::ZERO,
        scale: 1.0,
        image_size: Size::new(400.0, 300.0),
    });
    drag(&mut ed, at(150.0, 150.0), at(5000.0, 5000.0));
    let b = bounds(&ed, id);
    assert!(inside(b, 400.0, 300.0) && (b.max_x() - 400.0).abs() < 1e-6);

    ed.select_tool(Tool::Rectangle);
    drag(&mut ed, at(450.0, 100.0), at(480.0, 140.0));
    assert_eq!(ed.doc().shapes().len(), 1);
    assert!(matches!(ed.doc().shapes()[0].kind, ShapeKind::Geo(_)));
}
