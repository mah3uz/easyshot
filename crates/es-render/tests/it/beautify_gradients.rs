#![allow(clippy::unwrap_used)]

use es_model::{BackgroundColor, BackgroundGradient, BackgroundStyle, UnitPoint};
use es_render::beautify::GradientPreset;
use es_render::beautify::gradients::{PRESETS, render_background, render_gradient};
use tiny_skia::Pixmap;

fn rgb(p: &Pixmap, x: u32, y: u32) -> [f32; 3] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red() as f32, c.green() as f32, c.blue() as f32]
}

fn channels(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 255) as f32,
        ((hex >> 8) & 255) as f32,
        (hex & 255) as f32,
    ]
}

#[test]
fn a_plain_three_colour_gradient_hits_each_colour_at_its_position() {
    let colours = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)]
        .map(|(r, g, b)| BackgroundColor::custom(r, g, b));
    let g = BackgroundGradient {
        id: "plain".into(),
        title: "Plain".into(),
        colors: colours.to_vec(),
        start: UnitPoint { x: 0.5, y: 0.0 },
        end: UnitPoint { x: 0.5, y: 1.0 },
    };
    let p = render_gradient(&g, 100, 200).unwrap();
    let top = rgb(&p, 50, 0);
    assert!(top[0] > 252.0 && top[1] < 4.0 && top[2] < 4.0, "{top:?}");
    let mid = rgb(&p, 50, 100);
    assert!(mid[1] > 250.0 && mid[0] < 4.0 && mid[2] < 4.0, "{mid:?}");
    let end = rgb(&p, 50, 199);
    assert!(end[2] > 250.0 && end[0] < 4.0, "{end:?}");
}

type Expected = (&'static str, u32, u32, (f64, f64, f64, f64));

const EXPECTED: [Expected; 24] = [
    ("violet", 0x8728E2, 0x5105E0, (0.0, 0.0, 1.0, 1.0)),
    ("jade", 0x8CDFB8, 0x2C5447, (0.0, 0.0, 1.0, 1.0)),
    ("magenta", 0xF24DBD, 0xC0237C, (0.0, 0.0, 1.0, 1.0)),
    ("sky", 0x8FDFE3, 0x818BD6, (0.0, 0.0, 1.0, 1.0)),
    ("sunset", 0xF5A019, 0xF13612, (0.0, 0.0, 1.0, 1.0)),
    ("rose", 0xEBA1BB, 0xEC3D59, (0.0, 0.0, 1.0, 1.0)),
    ("orchid", 0xEE78B3, 0x7653A5, (0.0, 0.0, 1.0, 1.0)),
    ("ocean", 0x00A6CD, 0x004165, (0.0, 0.0, 1.0, 1.0)),
    ("periwinkle", 0x96A6EC, 0x5245A9, (0.5, 0.0, 0.5, 1.0)),
    ("crimson", 0x4A0404, 0xCB1616, (0.0, 1.0, 1.0, 0.0)),
    ("sandstone", 0xCDBE9B, 0x4F5E5A, (0.0, 0.0, 1.0, 1.0)),
    ("dusk", 0x6A59DB, 0xD24C8C, (0.0, 0.5, 1.0, 0.5)),
    ("steel", 0x42424B, 0xAFB1C2, (0.0, 0.5, 1.0, 0.5)),
    ("azure", 0x1252C4, 0x569BF3, (0.0, 0.5, 1.0, 0.5)),
    ("mist", 0x6D7DB2, 0xB6C3EF, (0.0, 0.5, 1.0, 0.5)),
    ("graphite", 0x1D1D21, 0x4F4F50, (0.0, 0.5, 1.0, 0.5)),
    ("indigo", 0x5F52DB, 0x383271, (0.5, 0.0, 0.5, 1.0)),
    ("charcoal", 0x444444, 0x232323, (0.5, 0.0, 0.5, 1.0)),
    ("amber", 0xF5AA19, 0xF37A16, (0.5, 0.0, 0.5, 1.0)),
    ("forest", 0x1A6440, 0x0F3F29, (0.5, 0.0, 0.5, 1.0)),
    ("silver", 0xABABAB, 0xEDEDED, (0.5, 0.0, 0.5, 1.0)),
    ("glacier", 0x617EA9, 0xC0D9E7, (0.5, 0.0, 0.5, 1.0)),
    ("ruby", 0xF55053, 0xC93335, (0.5, 0.0, 0.5, 1.0)),
    ("gold", 0xFFC009, 0xFFD42A, (0.5, 0.0, 0.5, 1.0)),
];

#[test]
fn the_presets_are_the_24_swatches_in_grid_order() {
    assert_eq!(PRESETS.len(), EXPECTED.len());
    for (preset, (id, from, to, (sx, sy, ex, ey))) in PRESETS.iter().zip(EXPECTED) {
        assert_eq!(preset.id, id);
        assert_eq!(preset.colors, [from, to], "{id}");
        assert_eq!((preset.start.x, preset.start.y), (sx, sy), "{id}");
        assert_eq!((preset.end.x, preset.end.y), (ex, ey), "{id}");
    }
}

#[test]
fn preset_ids_and_titles_are_unique_and_found_by_id() {
    for (i, preset) in PRESETS.iter().enumerate() {
        assert!(!preset.title.is_empty());
        assert_eq!(
            GradientPreset::by_id(preset.id).map(|p| p.id),
            Some(preset.id)
        );
        for other in &PRESETS[i + 1..] {
            assert_ne!(preset.id, other.id);
            assert_ne!(preset.title, other.title);
        }
    }
}

#[test]
fn every_preset_paints_its_first_colour_at_its_start_and_its_last_at_its_end() {
    let (w, h) = (64u32, 64u32);
    for preset in PRESETS {
        let p = render_gradient(&preset.style(), w, h).unwrap();
        let at = |u: es_model::UnitPoint| {
            let x = ((u.x * w as f64) as u32).min(w - 1);
            let y = ((u.y * h as f64) as u32).min(h - 1);
            rgb(&p, x, y)
        };
        for (got, hex) in [
            (at(preset.start), preset.colors[0]),
            (at(preset.end), preset.colors[1]),
        ] {
            let want = channels(hex);
            for c in 0..3 {
                assert!(
                    (got[c] - want[c]).abs() <= 8.0,
                    "{} {got:?} vs {want:?}",
                    preset.id
                );
            }
        }
    }
}

#[test]
fn a_wallpaper_without_an_image_falls_back_to_the_first_preset() {
    let style = BackgroundStyle::Wallpaper {
        path: "/missing.png".into(),
    };
    let fallback = render_background(&style, 32, 32).unwrap();
    let first = render_gradient(&PRESETS[0].style(), 32, 32).unwrap();
    assert_eq!(fallback.data(), first.data());
}

#[test]
fn a_solid_background_fills_every_pixel() {
    let style = BackgroundStyle::Solid(BackgroundColor::custom(0.2, 0.4, 0.6));
    let p = render_background(&style, 8, 8).unwrap();
    for (x, y) in [(0, 0), (7, 7), (3, 4)] {
        assert_eq!(rgb(&p, x, y), [51.0, 102.0, 153.0]);
    }
}

#[test]
fn no_background_stays_transparent() {
    let p = render_background(&BackgroundStyle::None, 4, 4).unwrap();
    assert_eq!(p.pixel(2, 2).unwrap().alpha(), 0);
}
