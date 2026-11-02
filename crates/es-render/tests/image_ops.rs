#![allow(clippy::unwrap_used)]

use es_render::image_ops::{ImageTransform as T, crop};
use image::{Rgba, RgbaImage};

fn sample(w: u32, h: u32) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| {
        Rgba([
            x as u8,
            y as u8,
            (x * 7 + y * 13) as u8,
            200 + (x % 50) as u8,
        ])
    })
}

#[test]
fn four_lefts_and_four_rights_are_identity() {
    let img = sample(37, 21);
    for t in [T::RotateLeft, T::RotateRight] {
        let mut cur = img.clone();
        for _ in 0..4 {
            cur = t.apply(&cur);
        }
        assert_eq!(cur.as_raw(), img.as_raw());
    }
}

#[test]
fn left_then_right_restores_the_image() {
    let img = sample(37, 21);
    let back = T::RotateRight.apply(&T::RotateLeft.apply(&img));
    assert_eq!(back.as_raw(), img.as_raw());
}

#[test]
fn flips_twice_are_identity() {
    let img = sample(37, 21);
    for t in [T::FlipHorizontal, T::FlipVertical] {
        assert_eq!(t.apply(&t.apply(&img)).as_raw(), img.as_raw());
    }
}

#[test]
fn pixels_land_where_the_editor_matrix_sends_them() {
    let (w, h) = (6u32, 4u32);
    let img = sample(w, h);
    let left = T::RotateLeft.apply(&img);
    let right = T::RotateRight.apply(&img);
    let fh = T::FlipHorizontal.apply(&img);
    let fv = T::FlipVertical.apply(&img);
    assert_eq!(
        (left.width(), left.height()),
        T::RotateLeft.output_size(w, h)
    );
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            assert_eq!(left.get_pixel(y, w - 1 - x), p);
            assert_eq!(right.get_pixel(h - 1 - y, x), p);
            assert_eq!(fh.get_pixel(w - 1 - x, y), p);
            assert_eq!(fv.get_pixel(x, h - 1 - y), p);
        }
    }
}

#[test]
fn semi_transparent_pixels_survive_exactly() {
    let mut img = sample(5, 5);
    img.put_pixel(2, 3, Rgba([10, 20, 30, 1]));
    let out = T::RotateLeft.apply(&T::FlipHorizontal.apply(&img));
    assert!(out.pixels().any(|p| *p == Rgba([10, 20, 30, 1])));
}

#[test]
fn crop_clamps_and_rejects_empty() {
    let img = sample(20, 10);
    let c = crop(&img, 5, 2, 10, 4).unwrap();
    assert_eq!((c.width(), c.height()), (10, 4));
    assert_eq!(c.get_pixel(0, 0), img.get_pixel(5, 2));
    let c = crop(&img, -5, -5, 12, 12).unwrap();
    assert_eq!((c.width(), c.height()), (7, 7));
    assert!(crop(&img, 25, 0, 5, 5).is_none());
    assert!(crop(&img, 0, 0, 0, 5).is_none());
}
