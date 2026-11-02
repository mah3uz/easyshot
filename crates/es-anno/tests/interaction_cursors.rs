mod common;

use common::*;
use es_anno::{CursorKind, Editor, Viewport};
use es_geom::{PVec, Point};
use es_model::AnnoShape;

fn resize_angle(c: CursorKind) -> f32 {
    match c {
        CursorKind::Resize { screen_angle_deg } => screen_angle_deg,
        other => panic!("not a resize cursor: {other:?}"),
    }
}

fn same_angle(a: f32, b: f32) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 1e-3 || d > 360.0 - 1e-3
}

fn cursor(ed: &Editor, p: PVec) -> CursorKind {
    ed.cursor_for(Point::new(p.x, p.y))
}

fn selected(shape: AnnoShape) -> Editor {
    let id = shape.id;
    let mut ed = editor(vec![shape], vec![]);
    select(&mut ed, &[id]);
    ed
}

/// The eight handles of a 100 x 100 frame at the local origin, with the screen angle each one
/// pulls at when the frame is upright.
const HANDLES: [((f64, f64), f32); 8] = [
    ((0.0, 0.0), 225.0),
    ((50.0, 0.0), 270.0),
    ((100.0, 0.0), 315.0),
    ((100.0, 50.0), 0.0),
    ((100.0, 100.0), 45.0),
    ((50.0, 100.0), 90.0),
    ((0.0, 100.0), 135.0),
    ((0.0, 50.0), 180.0),
];

#[test]
fn each_resize_handle_shows_the_direction_it_pulls_on_screen() {
    let ed = selected(rect(100.0, 100.0, 100.0, 100.0));
    for ((x, y), deg) in HANDLES {
        let c = cursor(&ed, PVec::new(100.0 + x, 100.0 + y));
        assert!(same_angle(resize_angle(c), deg), "{c:?} for {deg}");
    }
}

#[test]
fn a_rotated_shape_turns_its_resize_cursors_with_it() {
    let mut r = rect(100.0, 100.0, 100.0, 100.0);
    r.rotation = 30f64.to_radians();
    let ed = selected(r);
    for ((x, y), deg) in HANDLES {
        let at = PVec::add(
            PVec::new(100.0, 100.0),
            PVec::rot(PVec::new(x, y), 30f64.to_radians()),
        );
        let c = cursor(&ed, at);
        assert!(same_angle(resize_angle(c), deg + 30.0), "{c:?} for {deg}");
    }
}

#[test]
fn a_corner_reads_diagonal_however_wide_the_frame_and_mirroring_flips_it() {
    let ed = selected(rect(100.0, 100.0, 400.0, 40.0));
    let c = cursor(&ed, PVec::new(500.0, 140.0));
    assert!(same_angle(resize_angle(c), 45.0), "{c:?}");

    let mut m = rect(100.0, 100.0, 100.0, 100.0);
    m.mirrored = true;
    let ed = selected(m);
    let c = cursor(&ed, PVec::new(0.0, 200.0));
    assert!(same_angle(resize_angle(c), 135.0), "{c:?}");
}

#[test]
fn the_zoom_does_not_change_which_way_a_handle_points() {
    let mut ed = selected(rect(100.0, 100.0, 100.0, 100.0));
    ed.set_viewport(Viewport {
        origin: Point::new(30.0, 10.0),
        scale: 2.0,
        ..*ed.viewport()
    });
    let c = cursor(&ed, PVec::new(230.0, 210.0));
    assert!(same_angle(resize_angle(c), 225.0), "{c:?}");
}

#[test]
fn rotate_zones_show_the_rotate_cursor_for_their_corner() {
    let ed = selected(rect(100.0, 100.0, 100.0, 100.0));
    let d = 14.0 / 2f64.sqrt();
    let zones = [
        ((100.0 - d, 100.0 - d), 5),
        ((200.0 + d, 100.0 - d), 7),
        ((200.0 + d, 200.0 + d), 1),
        ((100.0 - d, 200.0 + d), 3),
    ];
    for ((x, y), octant) in zones {
        assert_eq!(cursor(&ed, PVec::new(x, y)), CursorKind::Rotate { octant });
    }

    let mut r = rect(100.0, 100.0, 100.0, 100.0);
    r.rotation = 45f64.to_radians();
    let ed = selected(r);
    assert_eq!(
        cursor(&ed, PVec::new(100.0, 86.0)),
        CursorKind::Rotate { octant: 6 },
        "the top-left corner of a square turned 45 degrees points straight up"
    );
}

#[test]
fn the_resize_cursor_holds_through_the_drag_and_the_rotate_cursor_follows_the_pointer() {
    let mut ed = selected(rect(100.0, 100.0, 100.0, 100.0));
    ed.pointer_down(at(200.0, 200.0));
    ed.pointer_move(at(50.0, 50.0));
    let c = cursor(&ed, PVec::new(50.0, 50.0));
    assert!(same_angle(resize_angle(c), 45.0), "{c:?}");
    ed.pointer_up(at(50.0, 50.0));

    let mut ed = selected(rect(100.0, 100.0, 100.0, 100.0));
    let d = 14.0 / 2f64.sqrt();
    ed.pointer_down(at(100.0 - d, 100.0 - d));
    assert_eq!(
        cursor(&ed, PVec::new(100.0 - d, 100.0 - d)),
        CursorKind::Rotate { octant: 5 }
    );
    ed.pointer_move(at(250.0, 150.0));
    assert_eq!(
        cursor(&ed, PVec::new(250.0, 150.0)),
        CursorKind::Rotate { octant: 0 }
    );
    ed.pointer_move(at(150.0, 250.0));
    assert_eq!(
        cursor(&ed, PVec::new(150.0, 250.0)),
        CursorKind::Rotate { octant: 2 }
    );
}

#[test]
fn arrow_ends_show_a_crosshair_and_the_bend_handle_a_resize_across_the_chord() {
    let ed = selected(arrow(100.0, 100.0, (0.0, 0.0), (200.0, 0.0)));
    assert_eq!(cursor(&ed, PVec::new(100.0, 100.0)), CursorKind::Crosshair);
    assert_eq!(cursor(&ed, PVec::new(300.0, 100.0)), CursorKind::Crosshair);
    let c = cursor(&ed, PVec::new(200.0, 100.0));
    assert!(same_angle(resize_angle(c).rem_euclid(180.0), 90.0), "{c:?}");

    let mut ed = selected(arrow(100.0, 100.0, (0.0, 0.0), (100.0, 100.0)));
    let c = cursor(&ed, PVec::new(150.0, 150.0));
    assert!(
        same_angle(resize_angle(c).rem_euclid(180.0), 135.0),
        "{c:?}"
    );
    ed.pointer_down(at(150.0, 150.0));
    ed.pointer_move(at(180.0, 120.0));
    assert_eq!(
        cursor(&ed, PVec::new(180.0, 120.0)),
        c,
        "kept while bending"
    );
    ed.pointer_up(at(180.0, 120.0));

    ed.pointer_down(at(100.0, 100.0));
    ed.pointer_move(at(400.0, 400.0));
    assert_eq!(cursor(&ed, PVec::new(400.0, 400.0)), CursorKind::Crosshair);
}

#[test]
fn a_handle_wins_over_the_shape_under_it_and_a_marquee_shows_the_arrow() {
    let r = rect(100.0, 100.0, 100.0, 100.0);
    let over = filled_rect(190.0, 190.0, 100.0, 100.0);
    let ri = r.id;
    let mut ed = editor(vec![r, over], vec![]);
    assert_eq!(cursor(&ed, PVec::new(200.0, 200.0)), CursorKind::OpenHand);
    select(&mut ed, &[ri]);
    let c = cursor(&ed, PVec::new(200.0, 200.0));
    assert!(same_angle(resize_angle(c), 45.0), "{c:?}");

    ed.pointer_down(at(600.0, 600.0));
    ed.pointer_move(at(150.0, 150.0));
    assert_eq!(cursor(&ed, PVec::new(150.0, 150.0)), CursorKind::Arrow);
}
