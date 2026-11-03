#![allow(clippy::unwrap_used)]

use es_render::encode::{PngProfile, encode_jpeg, encode_png};
use image::ColorType;
use tiny_skia::{Color, Pixmap};

fn half_red() -> Pixmap {
    let mut p = Pixmap::new(16, 16).unwrap();
    p.fill(Color::from_rgba8(255, 0, 0, 128));
    p
}

#[test]
fn png_keeps_alpha_and_straight_colour() {
    for profile in [PngProfile::Fast, PngProfile::Small] {
        let bytes = encode_png(&half_red(), profile).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().into_rgba8();
        let px = img.get_pixel(3, 3).0;
        assert_eq!(px[3], 128, "{profile:?}");
        assert!(
            px[0] >= 253 && px[1] == 0,
            "red survives unpremultiplying: {px:?}"
        );
    }
}

#[test]
fn small_profile_is_never_larger_than_fast() {
    let mut p = Pixmap::new(300, 300).unwrap();
    for (i, px) in p.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let v = ((i % 300) / 3) as u8;
        px.copy_from_slice(&[v, v / 2, 255 - v, 255]);
    }
    let fast = encode_png(&p, PngProfile::Fast).unwrap();
    let small = encode_png(&p, PngProfile::Small).unwrap();
    assert!(
        small.len() <= fast.len(),
        "{} vs {}",
        small.len(),
        fast.len()
    );
}

#[test]
fn jpeg_has_no_alpha_and_transparent_pixels_become_white() {
    let transparent = Pixmap::new(16, 16).unwrap();
    let bytes = encode_jpeg(&transparent, 90).unwrap();
    let img = image::load_from_memory(&bytes).unwrap();
    assert_eq!(img.color(), ColorType::Rgb8);
    let px = img.to_rgb8().get_pixel(8, 8).0;
    assert!(px.iter().all(|&c| c >= 250), "{px:?}");
}

#[test]
fn jpeg_flattens_half_transparent_red_to_pink_not_dark_red() {
    let bytes = encode_jpeg(&half_red(), 95).unwrap();
    let px = image::load_from_memory(&bytes)
        .unwrap()
        .to_rgb8()
        .get_pixel(8, 8)
        .0;
    assert!(px[0] >= 245, "{px:?}");
    assert!((110..150).contains(&px[1]), "green is about half: {px:?}");
}

#[test]
fn jpeg_quality_trades_size() {
    let mut p = Pixmap::new(128, 128).unwrap();
    for (i, px) in p.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let n = (i.wrapping_mul(2654435761) >> 24) as u8;
        px.copy_from_slice(&[n, n.wrapping_mul(3), n.wrapping_mul(7), 255]);
    }
    let low = encode_jpeg(&p, 20).unwrap();
    let high = encode_jpeg(&p, 95).unwrap();
    assert!(low.len() < high.len());
}
