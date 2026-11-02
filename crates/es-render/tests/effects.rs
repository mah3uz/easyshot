#![allow(clippy::unwrap_used)]

use es_anno::draw::RedactionKind;
use es_geom::Rect;
use es_render::effects::{Patch, blur_radius, pixel_block, redaction_patch};
use tiny_skia::{Color, Pixmap, PremultipliedColorU8};

fn noise(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    let mut s = 12345u32;
    for px in p.pixels_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        let v = (s >> 24) as u8;
        *px = PremultipliedColorU8::from_rgba(v, v ^ 0x55, 255 - v, 255).unwrap();
    }
    p
}

fn alpha(p: &Patch, x: i32, y: i32) -> u8 {
    p.pixmap
        .pixel((x - p.x) as u32, (y - p.y) as u32)
        .unwrap()
        .alpha()
}

fn patch(
    base: &Pixmap,
    rect: Rect,
    rot: f64,
    mirrored: bool,
    kind: RedactionKind,
    d: f64,
) -> Patch {
    redaction_patch(base, rect, rot, mirrored, kind, d, 1.0).unwrap()
}

#[test]
fn formulas_map_density_to_radius_and_block() {
    assert_eq!(blur_radius(0.0), 2.0);
    assert_eq!(blur_radius(1.0), 30.0);
    assert_eq!(pixel_block(0.0), 4.0);
    assert_eq!(pixel_block(1.0), 40.0);
}

#[test]
fn patch_is_transparent_outside_a_rotated_rect() {
    let base = noise(200, 200);
    let rect = Rect::new(100.0, 40.0, 180.0, 70.0);
    let p = patch(
        &base,
        rect,
        std::f64::consts::FRAC_PI_4,
        false,
        RedactionKind::Pixelate,
        0.5,
    );
    assert_eq!(
        alpha(&p, p.x, p.y),
        0,
        "the bounding-box corner is outside the shape"
    );
    assert_eq!(alpha(&p, 130, 75), 255);
    assert!(p.pixmap.width() > 60);
}

#[test]
fn mirroring_moves_the_rect_to_the_other_side_of_its_origin() {
    let base = noise(200, 100);
    let rect = Rect::new(100.0, 20.0, 160.0, 60.0);
    let plain = patch(&base, rect, 0.0, false, RedactionKind::Pixelate, 0.5);
    let mirrored = patch(&base, rect, 0.0, true, RedactionKind::Pixelate, 0.5);
    assert_eq!(plain.x, 100);
    assert_eq!(mirrored.x, 40);
}

#[test]
fn pixelate_makes_flat_blocks_sized_by_density() {
    let base = noise(120, 120);
    let rect = Rect::new(0.0, 0.0, 80.0, 80.0);
    let run = |p: &Patch| {
        let row: Vec<_> = (0..80).map(|x| p.pixmap.pixel(x, 0).unwrap()).collect();
        row.iter().take_while(|c| **c == row[0]).count()
    };
    assert_eq!(
        run(&patch(
            &base,
            rect,
            0.0,
            false,
            RedactionKind::Pixelate,
            1.0
        )),
        40
    );
    assert_eq!(
        run(&patch(
            &base,
            rect,
            0.0,
            false,
            RedactionKind::Pixelate,
            0.0
        )),
        4
    );
}

#[test]
fn pixelate_averages_each_block() {
    let mut base = Pixmap::new(8, 8).unwrap();
    base.fill(Color::BLACK);
    for y in 0..8usize {
        for x in 0..4usize {
            base.pixels_mut()[y * 8 + x] =
                PremultipliedColorU8::from_rgba(255, 255, 255, 255).unwrap();
        }
    }
    let p = patch(
        &base,
        Rect::new(0.0, 0.0, 8.0, 8.0),
        0.0,
        false,
        RedactionKind::Pixelate,
        0.0,
    );
    assert_eq!(p.pixmap.pixel(0, 0).unwrap().red(), 255);
    assert_eq!(p.pixmap.pixel(7, 7).unwrap().red(), 0);
}

#[test]
fn blur_softens_a_sharp_edge_and_keeps_flat_areas() {
    let mut base = Pixmap::new(100, 60).unwrap();
    base.fill(Color::BLACK);
    for y in 0..60usize {
        for x in 50..100usize {
            base.pixels_mut()[y * 100 + x] =
                PremultipliedColorU8::from_rgba(255, 255, 255, 255).unwrap();
        }
    }
    let p = patch(
        &base,
        Rect::new(10.0, 10.0, 90.0, 50.0),
        0.0,
        false,
        RedactionKind::Blur,
        0.0,
    );
    let at = |x: u32| p.pixmap.pixel(x - p.x as u32, 20).unwrap().red();
    let edge = at(50);
    assert!(
        edge > 60 && edge < 200,
        "edge pixel is mid-grey, got {edge}"
    );
    assert!(at(49) > 0 && at(51) < 255);
    assert!(at(12) < 2 && at(88) > 253, "far from the edge stays flat");
}

#[test]
fn rect_outside_the_image_gives_no_patch() {
    let base = noise(50, 50);
    let r = Rect::new(80.0, 80.0, 120.0, 120.0);
    assert!(redaction_patch(&base, r, 0.0, false, RedactionKind::Blur, 0.5, 1.0).is_none());
}
