#![allow(clippy::unwrap_used)]

use es_anno::draw::DrawList;
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::compose_with_wallpaper;
use es_render::beautify::wallpaper::{decode_wallpaper, draw_wallpaper};
use std::path::PathBuf;
use tiny_skia::{Color, Pixmap};

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn vertical_ramp(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    for y in 0..h {
        let v = (y * 255 / (h - 1)) as u8;
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            p.data_mut()[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    p
}

fn wallpaper_settings() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Wallpaper {
            path: PathBuf::from("w.png"),
        },
        padding: 0.5,
        shadow: 0.0,
        ..BackgroundSettings::default()
    }
}

#[test]
fn a_square_wallpaper_on_a_wide_canvas_is_cropped_not_stretched() {
    let wall = vertical_ramp(100, 100);
    let mut canvas = Pixmap::new(200, 100).unwrap();
    draw_wallpaper(&mut canvas, &wall);
    let top = i32::from(px(&canvas, 100, 0)[0]);
    let mid = i32::from(px(&canvas, 100, 50)[0]);
    assert!(
        (top - 64).abs() <= 6,
        "top row shows a quarter of the ramp, got {top}"
    );
    assert!(
        (mid - 128).abs() <= 6,
        "middle row shows the middle, got {mid}"
    );
}

#[test]
fn a_wide_wallpaper_on_a_square_canvas_keeps_its_centre() {
    let mut wall = Pixmap::new(200, 100).unwrap();
    wall.fill(Color::from_rgba8(255, 0, 0, 255));
    for y in 0..100 {
        for x in 100..200 {
            let i = ((y * 200 + x) * 4) as usize;
            wall.data_mut()[i..i + 4].copy_from_slice(&[0, 0, 255, 255]);
        }
    }
    let mut canvas = Pixmap::new(100, 100).unwrap();
    draw_wallpaper(&mut canvas, &wall);
    assert_eq!(px(&canvas, 10, 50), [255, 0, 0, 255]);
    assert_eq!(px(&canvas, 90, 50), [0, 0, 255, 255]);
}

#[test]
fn the_wallpaper_fills_the_whole_stage_around_the_card() {
    let wall = vertical_ramp(64, 64);
    let image = Pixmap::new(40, 40).unwrap();
    let out = compose_with_wallpaper(
        &image,
        &DrawList::default(),
        &wallpaper_settings(),
        Some(&wall),
    )
    .unwrap();
    let (w, h) = (out.width() - 1, out.height() - 1);
    for (x, y) in [(0, 0), (w, 0), (0, h), (w, h)] {
        assert_eq!(px(&out, x, y)[3], 255);
    }
}

#[test]
fn a_missing_wallpaper_falls_back_to_a_light_look_not_black() {
    let image = Pixmap::new(40, 40).unwrap();
    let out =
        compose_with_wallpaper(&image, &DrawList::default(), &wallpaper_settings(), None).unwrap();
    let [r, g, b, a] = px(&out, 1, 1);
    assert_eq!(a, 255);
    assert!(
        u32::from(r) + u32::from(g) + u32::from(b) > 600,
        "{r} {g} {b}"
    );
}

#[test]
fn encoded_wallpaper_bytes_decode_to_the_same_pixels() {
    let wall = vertical_ramp(8, 8);
    let bytes = es_render::encode::encode_png(&wall, es_render::encode::PngProfile::Fast).unwrap();
    let back = decode_wallpaper(&bytes).unwrap();
    assert_eq!((back.width(), back.height()), (8, 8));
    assert_eq!(px(&back, 3, 7), px(&wall, 3, 7));
    assert!(decode_wallpaper(b"not an image").is_none());
}
