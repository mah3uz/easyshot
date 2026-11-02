#![allow(clippy::unwrap_used)]

use es_model::{BackgroundColor, BackgroundGradient, BackgroundStyle, UnitPoint};
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

fn stop_colour(stops: &[(u32, f32); 3], t: f32) -> [f32; 3] {
    let i = if t <= stops[1].1 { 0 } else { 1 };
    let (a, b) = (stops[i], stops[i + 1]);
    let k = ((t - a.1) / (b.1 - a.1)).clamp(0.0, 1.0);
    let (ca, cb) = (channels(a.0), channels(b.0));
    [0, 1, 2].map(|c| ca[c] + (cb[c] - ca[c]) * k)
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

#[test]
fn every_preset_follows_its_stops_and_only_lightens_them_with_the_glows() {
    let (w, h) = (400u32, 300u32);
    for preset in PRESETS {
        let p = render_gradient(&preset.style(), w, h).unwrap();
        let max_lift = preset.highlights.iter().map(|g| g.opacity).sum::<f32>();
        for y in [0, h / 2, h - 1] {
            let t = (y as f32 + 0.5) / h as f32;
            let base = stop_colour(&preset.stops, t);
            let got = rgb(&p, w / 2, y);
            for c in 0..3 {
                let ceiling = base[c] + (255.0 - base[c]) * max_lift + 3.0;
                assert!(
                    got[c] >= base[c] - 3.0 && got[c] <= ceiling,
                    "{} y={y} channel {c}: {} vs base {}",
                    preset.id,
                    got[c],
                    base[c]
                );
            }
        }
    }
}

#[test]
fn the_first_glow_lightens_its_centre_by_about_its_opacity() {
    let (w, h) = (400u32, 300u32);
    for preset in PRESETS {
        let glow = preset.highlights[0];
        let p = render_gradient(&preset.style(), w, h).unwrap();
        let (x, y) = ((glow.x * w as f32) as u32, (glow.y * h as f32) as u32);
        let base = stop_colour(&preset.stops, glow.y);
        let got = rgb(&p, x, y);
        let lift = (255.0 - base[2]) * glow.opacity;
        assert!(
            got[2] - base[2] >= lift * 0.9 - 2.0,
            "{}: lifted {} wanted about {lift}",
            preset.id,
            got[2] - base[2]
        );
    }
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
