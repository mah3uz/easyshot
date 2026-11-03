use es_backend::wayland::normalise_frame;
use es_geom::Rect;
use es_model::platform::{Frame, Transform};
use std::time::Instant;

const UPRIGHT: [&[u8]; 2] = [&[1, 2, 3], &[4, 5, 6]];

fn frame(rows: &[&[u8]], transform: Transform) -> Frame {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let bgra: Vec<u8> = rows
        .iter()
        .flat_map(|r| r.iter())
        .flat_map(|&v| [v, 10 * v, 0, 255])
        .collect();
    Frame {
        output: None,
        size_px: (w, h),
        logical: Rect::new(0.0, 0.0, 3.0, 2.0),
        scale: 1.0,
        bgra: bgra.into(),
        transform,
        captured_at: Instant::now(),
    }
}

// Each buffer is the upright 3x2 image as the compositor stores it: mirrored, then turned counter-clockwise.
fn check(transform: Transform, buffer: &[&[u8]]) {
    let upright = normalise_frame(frame(buffer, transform));
    let expected = frame(&UPRIGHT, Transform::Normal);
    assert_eq!(upright.size_px, (3, 2), "{transform:?}");
    assert_eq!(upright.transform, Transform::Normal, "{transform:?}");
    assert_eq!(upright.logical, expected.logical, "{transform:?}");
    assert_eq!(upright.bgra, expected.bgra, "{transform:?}");
}

#[test]
fn normal_is_untouched() {
    check(Transform::Normal, &UPRIGHT);
}

#[test]
fn r90() {
    check(Transform::R90, &[&[3, 6], &[2, 5], &[1, 4]]);
}

#[test]
fn r180() {
    check(Transform::R180, &[&[6, 5, 4], &[3, 2, 1]]);
}

#[test]
fn r270() {
    check(Transform::R270, &[&[4, 1], &[5, 2], &[6, 3]]);
}

#[test]
fn flipped() {
    check(Transform::Flipped, &[&[3, 2, 1], &[6, 5, 4]]);
}

#[test]
fn f90() {
    check(Transform::F90, &[&[1, 4], &[2, 5], &[3, 6]]);
}

#[test]
fn f180() {
    check(Transform::F180, &[&[4, 5, 6], &[1, 2, 3]]);
}

#[test]
fn f270() {
    check(Transform::F270, &[&[6, 3], &[5, 2], &[4, 1]]);
}

#[test]
fn a_buffer_of_the_wrong_length_is_left_alone() {
    let mut f = frame(&[&[3, 6], &[2, 5], &[1, 4]], Transform::R90);
    f.bgra = vec![0u8; 5].into();
    let out = normalise_frame(f);
    assert_eq!(out.size_px, (2, 3));
    assert_eq!(out.transform, Transform::R90);
}
