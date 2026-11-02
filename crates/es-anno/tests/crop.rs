mod common;

use common::{FakeMeasure, arrow, filled_rect, model, redaction, spotlight, text};
use es_anno::Document;
use es_anno::crop::{AspectPreset, CropHandle, CropMap, CropState, ImageOp, MIN_CROP_PIXELS};
use es_geom::{PVec, Point, Rect, Size};
use es_model::{AnnoShape, DrawProps, NumberedProps, ShapeKind};
use kurbo::Vec2;
use std::f64::consts::TAU;

const OPS: [ImageOp; 4] = [
    ImageOp::RotateLeft,
    ImageOp::RotateRight,
    ImageOp::FlipHorizontal,
    ImageOp::FlipVertical,
];

fn state() -> CropState {
    CropState::new(Size::new(1000.0, 500.0))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_new_crop_covers_the_whole_image_and_applies_to_nothing() {
    let s = state();
    assert!(s.is_full());
    assert_eq!(s.rect(), Rect::new(0.0, 0.0, 1000.0, 500.0));
    assert!(s.apply().is_none());
}

#[test]
fn an_edge_cannot_be_dragged_past_the_minimum_size() {
    let mut s = state();
    s.resize(CropHandle::Right, Point::new(10.0, 250.0), false);
    assert_eq!(s.rect().width(), MIN_CROP_PIXELS);
    s.resize(CropHandle::Top, Point::new(10.0, 9999.0), false);
    assert_eq!(s.rect().height(), MIN_CROP_PIXELS);
}

#[test]
fn a_drag_outside_the_image_is_clamped_to_it() {
    let mut s = state();
    s.resize(CropHandle::TopLeft, Point::new(-80.0, -40.0), false);
    assert_eq!(s.rect(), Rect::new(0.0, 0.0, 1000.0, 500.0));
    s.resize(CropHandle::BottomRight, Point::new(5000.0, 5000.0), false);
    assert_eq!(s.rect(), Rect::new(0.0, 0.0, 1000.0, 500.0));
}

#[test]
fn choosing_an_aspect_refits_the_largest_rect_around_the_centre() {
    let mut s = state();
    s.set_aspect(AspectPreset::Square);
    assert_eq!(s.rect(), Rect::new(250.0, 0.0, 750.0, 500.0));
    s.set_aspect(AspectPreset::Landscape16x9);
    let r = s.rect();
    assert!(close(r.width() / r.height(), 16.0 / 9.0));
    assert!(r.x0 >= 0.0 && r.x1 <= 1000.0 && r.y0 >= 0.0 && r.y1 <= 500.0);
}

#[test]
fn original_aspect_keeps_the_image_ratio_after_a_shrink() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(600.0, 100.0), false);
    s.set_aspect(AspectPreset::Original);
    let r = s.rect();
    assert!(close(r.width() / r.height(), 2.0));
}

#[test]
fn a_locked_corner_drag_keeps_the_ratio_and_stays_inside_the_image() {
    let mut s = state();
    s.set_aspect(AspectPreset::Square);
    s.resize(CropHandle::TopLeft, Point::new(300.0, 100.0), false);
    let r = s.rect();
    assert!(close(r.width(), r.height()));
    assert!(close(r.x1, 750.0) && close(r.y1, 500.0));
    s.resize(CropHandle::BottomRight, Point::new(5000.0, 5000.0), false);
    let r = s.rect();
    assert!(close(r.width(), r.height()));
    assert!(r.x1 <= 1000.0 && r.y1 <= 500.0);
}

#[test]
fn edge_drags_ignore_a_locked_aspect_and_its_handles_are_hidden() {
    let mut s = state();
    s.set_aspect(AspectPreset::Square);
    s.resize(CropHandle::Right, Point::new(600.0, 250.0), false);
    assert_ne!(s.rect().width(), s.rect().height());
    assert!(!CropHandle::Left.visible(s.aspect()));
    assert!(CropHandle::TopLeft.visible(s.aspect()));
    assert!(CropHandle::Left.visible(AspectPreset::Freeform));
}

#[test]
fn resizing_from_the_centre_keeps_the_centre_fixed() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(800.0, 400.0), false);
    s.resize(CropHandle::TopLeft, Point::new(200.0, 100.0), false);
    let c = s.rect().center();
    s.resize(
        CropHandle::BottomRight,
        Point::new(c.x + 100.0, c.y + 50.0),
        true,
    );
    assert_eq!(s.rect().center(), c);
    assert!(close(s.rect().width(), 200.0) && close(s.rect().height(), 100.0));
    s.resize(CropHandle::Right, Point::new(c.x + 3.0, c.y), true);
    assert!(close(s.rect().width(), MIN_CROP_PIXELS));
    assert_eq!(s.rect().center(), c);
}

#[test]
fn centre_resize_cannot_leave_the_image_on_the_nearer_side() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(300.0, 300.0), false);
    s.resize(CropHandle::TopLeft, Point::new(100.0, 100.0), false);
    s.resize(CropHandle::BottomRight, Point::new(900.0, 900.0), true);
    let r = s.rect();
    assert!(r.x0 >= 0.0 && r.y0 >= 0.0 && r.x1 <= 1000.0 && r.y1 <= 500.0);
}

#[test]
fn moving_keeps_the_size_and_stays_inside_the_image() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(400.0, 300.0), false);
    s.move_by(Vec2::new(900.0, -50.0));
    assert_eq!(s.rect(), Rect::new(600.0, 0.0, 1000.0, 300.0));
}

#[test]
fn reset_restores_the_whole_image_or_the_largest_rect_of_the_locked_aspect() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(400.0, 300.0), false);
    s.reset();
    assert!(s.is_full());
    s.set_aspect(AspectPreset::Square);
    s.resize(CropHandle::BottomRight, Point::new(300.0, 300.0), false);
    s.reset();
    assert_eq!(s.rect(), Rect::new(250.0, 0.0, 750.0, 500.0));
}

#[test]
fn cancel_leaves_no_trace_of_the_edit() {
    let mut s = state();
    s.set_aspect(AspectPreset::Square);
    s.cancel();
    assert!(s.is_full());
    assert_eq!(s.aspect(), AspectPreset::Freeform);
}

#[test]
fn applying_a_whole_pixel_crop_reports_its_region_and_keeps_scale_one() {
    let mut s = state();
    s.resize(CropHandle::TopLeft, Point::new(100.0, 50.0), false);
    s.resize(CropHandle::BottomRight, Point::new(700.0, 450.0), false);
    let done = s.apply().expect("a real crop");
    let r = done.region;
    assert_eq!((r.x, r.y, r.width, r.height), (100, 50, 600, 400));
    assert_eq!((done.map.sx, done.map.sy), (1.0, 1.0));
    assert_eq!(s.output_size(), (600, 400));
}

#[test]
fn a_fractional_crop_rescales_by_the_rounding_error_only() {
    let mut s = state();
    s.resize(CropHandle::BottomRight, Point::new(600.4, 300.0), false);
    let done = s.apply().expect("a real crop");
    assert_eq!(done.region.width, 600);
    assert!(close(done.map.sx, 600.0 / 600.4));
}

fn sample() -> Vec<AnnoShape> {
    let mut rotated = filled_rect(300.0, 120.0, 80.0, 40.0);
    rotated.rotation = 0.4;
    let mut ink = AnnoShape::new(
        50.0,
        60.0,
        ShapeKind::Draw(DrawProps {
            points: (0..40)
                .map(|i| PVec::new(f64::from(i) * 3.0, f64::from(i % 7) * 4.0))
                .collect(),
            is_complete: true,
            ..DrawProps::default()
        }),
    );
    ink.rotation = -0.2;
    let mut badge = AnnoShape::new(500.0, 200.0, ShapeKind::Numbered(NumberedProps::default()));
    badge.mirrored = true;
    vec![
        rotated,
        ink,
        badge,
        arrow(100.0, 300.0, (0.0, 0.0), (140.0, 60.0)),
        text(700.0, 100.0, "hello"),
        redaction(20.0, 400.0),
        spotlight(600.0, 300.0),
    ]
}

fn document(shapes: Vec<AnnoShape>) -> Document {
    Document::new(model(shapes, vec![]), Box::new(FakeMeasure))
}

fn page_vertices(doc: &Document, shape: &AnnoShape) -> Vec<PVec> {
    let g = doc.geometry(shape.id).expect("geometry");
    es_anno::document::page_transform(shape).apply_to_points(g.vertices())
}

fn same_place(a: &[AnnoShape], b: &[AnnoShape], tol: f64, expect: impl Fn(PVec) -> PVec) {
    let (da, db) = (document(a.to_vec()), document(b.to_vec()));
    for (old, new) in a.iter().zip(b) {
        let want: Vec<PVec> = page_vertices(&da, old).into_iter().map(&expect).collect();
        let got = page_vertices(&db, new);
        assert_eq!(want.len(), got.len());
        for (w, g) in want.iter().zip(&got) {
            assert!(
                (w.x - g.x).abs() < tol && (w.y - g.y).abs() < tol,
                "{w:?} vs {g:?}"
            );
        }
    }
}

#[test]
fn a_crop_keeps_every_annotation_over_the_same_image_pixels() {
    let before = sample();
    let mut after = before.clone();
    let mut s = state();
    s.resize(CropHandle::TopLeft, Point::new(40.0, 30.0), false);
    s.resize(CropHandle::BottomRight, Point::new(840.0, 470.0), false);
    let map = s.apply().expect("a crop").map;
    map.apply_to(&mut after);
    same_place(&before, &after, 1e-6, |v| {
        PVec::new(v.x - map.origin.x, v.y - map.origin.y)
    });
}

#[test]
fn cropping_then_applying_the_inverse_map_restores_every_shape() {
    let original = sample();
    let mut shapes = original.clone();
    let map = CropMap {
        origin: Point::new(40.5, 30.25),
        sx: 1.0017,
        sy: 0.9983,
    };
    map.apply_to(&mut shapes);
    assert_ne!(shapes, original);
    map.inverse().apply_to(&mut shapes);
    let (kept, back): (Vec<_>, Vec<_>) = original
        .iter()
        .cloned()
        .zip(shapes.iter().cloned())
        .filter(|(o, _)| !o.is_text())
        .unzip();
    same_place(&kept, &back, 1e-3, |v| v);
    for (o, r) in original.iter().zip(&shapes) {
        assert!(close(o.x, r.x) && close(o.y, r.y));
    }
}

#[test]
fn undoing_a_crop_through_the_document_snapshot_restores_the_exact_coordinates() {
    let mut doc = document(sample());
    let before = doc.snapshot();
    let mut after = before.clone();
    let mut s = state();
    s.resize(CropHandle::TopLeft, Point::new(100.0, 100.0), false);
    s.apply().expect("a crop").map.apply_to(&mut after.shapes);
    doc.restore(after.clone());
    assert!(doc.matches(&after) && !doc.matches(&before));
    doc.restore(before.clone());
    assert!(doc.matches(&before));
}

#[test]
fn every_image_op_carries_each_vertex_to_where_its_pixel_moved() {
    let size = Size::new(1000.0, 500.0);
    for op in OPS {
        let before = sample();
        let mut after = before.clone();
        op.apply_to(size, &mut after);
        let m = op.matrix(size);
        same_place(&before, &after, 1e-6, |v| m.apply_to_point(v));
    }
}

#[test]
fn rotations_swap_the_size_and_flips_do_not() {
    let size = Size::new(1000.0, 500.0);
    assert_eq!(
        ImageOp::RotateLeft.size_after(size),
        Size::new(500.0, 1000.0)
    );
    assert_eq!(
        ImageOp::RotateRight.size_after(size),
        Size::new(500.0, 1000.0)
    );
    assert_eq!(ImageOp::FlipHorizontal.size_after(size), size);
    assert_eq!(ImageOp::FlipVertical.size_after(size), size);
}

#[test]
fn an_op_followed_by_its_inverse_restores_position_and_orientation() {
    let size = Size::new(1000.0, 500.0);
    for op in OPS {
        let original = sample();
        let mut shapes = original.clone();
        op.apply_to(size, &mut shapes);
        op.inverse().apply_to(op.size_after(size), &mut shapes);
        for (o, r) in original.iter().zip(&shapes) {
            assert!(close(o.x, r.x) && close(o.y, r.y), "{op:?}");
            assert_eq!(o.mirrored, r.mirrored, "{op:?}");
            let turn = (o.rotation - r.rotation).rem_euclid(TAU);
            assert!(turn < 1e-9 || TAU - turn < 1e-9, "{op:?}");
        }
    }
}

#[test]
fn a_flip_toggles_mirroring_and_a_rotation_does_not() {
    let size = Size::new(1000.0, 500.0);
    let mut shapes = vec![filled_rect(10.0, 20.0, 30.0, 30.0)];
    ImageOp::RotateLeft.apply_to(size, &mut shapes);
    assert!(!shapes[0].mirrored);
    ImageOp::FlipHorizontal.apply_to(ImageOp::RotateLeft.size_after(size), &mut shapes);
    assert!(shapes[0].mirrored);
}
