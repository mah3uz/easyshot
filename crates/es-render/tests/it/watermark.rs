#![allow(clippy::unwrap_used)]

use es_geom::Rgba;
use es_model::WatermarkSettings;
use es_render::beautify::watermark::{paint, paint_with};
use es_text::{BUNDLED, FontSet};
use tiny_skia::{Color, Pixmap};

fn white(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    p.fill(Color::WHITE);
    p
}

fn settings(text: &str) -> WatermarkSettings {
    WatermarkSettings {
        is_enabled: true,
        text: text.into(),
        density: 2.0,
        font_size: 24.0,
        rotation_degrees: 0.0,
        opacity: 0.5,
        color: Rgba::new(0.0, 0.0, 0.0, 1.0),
    }
}

fn inked(p: &Pixmap, x0: u32, y0: u32, x1: u32, y1: u32) -> bool {
    (y0..y1).any(|y| (x0..x1).any(|x| p.pixel(x, y).unwrap().red() < 250))
}

#[test]
fn empty_or_blank_text_paints_nothing() {
    for text in ["", "   \n"] {
        let mut p = white(200, 200);
        paint(&mut p, &settings(text));
        assert!(!inked(&p, 0, 0, 200, 200), "{text:?}");
    }
}

#[test]
fn zero_opacity_paints_nothing() {
    let mut p = white(200, 200);
    let s = WatermarkSettings {
        opacity: 0.0,
        ..settings("mark")
    };
    paint(&mut p, &s);
    assert!(!inked(&p, 0, 0, 200, 200));
}

#[test]
fn tiles_sit_on_rows_of_a_height_divided_by_density() {
    let mut p = white(400, 400);
    paint(&mut p, &settings("MARK"));
    assert!(
        inked(&p, 150, 85, 250, 115),
        "a tile is centred on x 200, y 100"
    );
    assert!(inked(&p, 150, 285, 250, 315), "and on y 300");
    assert!(!inked(&p, 0, 180, 400, 220), "nothing between the rows");
    assert!(
        !inked(&p, 0, 85, 100, 115),
        "columns are 2.2 rows apart, so the left edge is bare"
    );
}

#[test]
fn opacity_is_capped_at_three_quarters() {
    let mut p = white(400, 400);
    let s = WatermarkSettings {
        opacity: 5.0,
        font_size: 120.0,
        ..settings("M")
    };
    paint(&mut p, &s);
    let darkest = (0..400)
        .flat_map(|y| (0..400).map(move |x| (x, y)))
        .map(|(x, y)| p.pixel(x, y).unwrap().red())
        .min()
        .unwrap();
    assert!(
        darkest >= 60,
        "black at 0.75 over white is 64, got {darkest}"
    );
    assert!(darkest < 90);
}

#[test]
fn rotation_turns_the_tiles_about_the_canvas_centre() {
    let mut flat = white(400, 400);
    let mut turned = white(400, 400);
    paint(&mut flat, &settings("MARK"));
    let s = WatermarkSettings {
        rotation_degrees: 90.0,
        ..settings("MARK")
    };
    paint(&mut turned, &s);
    assert_ne!(flat.data(), turned.data());
    assert!(!inked(&flat, 285, 160, 315, 240));
    assert!(
        inked(&turned, 285, 160, 315, 240),
        "the tile at x 200, y 100 swings to x 300, y 200"
    );
}

const BENGALI: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/FallbackBengali.ttf");

fn with_bengali() -> FontSet {
    FontSet::from_data(BUNDLED.iter().map(|b| b.to_vec()).chain([BENGALI.to_vec()]))
}

#[test]
fn text_the_bundled_faces_lack_draws_from_an_installed_font_not_as_boxes() {
    let s = WatermarkSettings {
        font_size: 60.0,
        ..settings("বাংলা")
    };
    let mut drawn = white(400, 400);
    paint_with(&mut drawn, &s, with_bengali);
    let mut boxes = white(400, 400);
    paint_with(&mut boxes, &s, FontSet::bundled);
    assert!(inked(&drawn, 100, 70, 300, 130));
    assert_ne!(drawn.data(), boxes.data());
}

#[test]
fn text_the_bundled_faces_cover_never_reads_the_installed_fonts() {
    let mut p = white(200, 200);
    paint_with(&mut p, &settings("MARK"), || {
        panic!("the installed fonts were read for bundled text")
    });
    assert!(inked(&p, 0, 0, 200, 200));
}
