use crate::beautify::wallpaper::draw_wallpaper;
use es_model::{BackgroundColor, BackgroundGradient, BackgroundStyle, UnitPoint};
use rayon::prelude::*;
use tiny_skia::{Color, Pixmap};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub colors: [u32; 2],
    pub start: UnitPoint,
    pub end: UnitPoint,
}

type Axis = (UnitPoint, UnitPoint);

const fn at(x: f64, y: f64) -> UnitPoint {
    UnitPoint { x, y }
}

const VERTICAL: Axis = (at(0.5, 0.0), at(0.5, 1.0));
const HORIZONTAL: Axis = (at(0.0, 0.5), at(1.0, 0.5));
const DIAGONAL: Axis = (at(0.0, 0.0), at(1.0, 1.0));
const RISING: Axis = (at(0.0, 1.0), at(1.0, 0.0));

const fn preset(
    id: &'static str,
    title: &'static str,
    colors: [u32; 2],
    axis: Axis,
) -> GradientPreset {
    GradientPreset {
        id,
        title,
        colors,
        start: axis.0,
        end: axis.1,
    }
}

pub const PRESETS: [GradientPreset; 24] = [
    preset("violet", "Violet", [0x8728E2, 0x5105E0], DIAGONAL),
    preset("jade", "Jade", [0x8CDFB8, 0x2C5447], DIAGONAL),
    preset("magenta", "Magenta", [0xF24DBD, 0xC0237C], DIAGONAL),
    preset("sky", "Sky", [0x8FDFE3, 0x818BD6], DIAGONAL),
    preset("sunset", "Sunset", [0xF5A019, 0xF13612], DIAGONAL),
    preset("rose", "Rose", [0xEBA1BB, 0xEC3D59], DIAGONAL),
    preset("orchid", "Orchid", [0xEE78B3, 0x7653A5], DIAGONAL),
    preset("ocean", "Ocean", [0x00A6CD, 0x004165], DIAGONAL),
    preset("periwinkle", "Periwinkle", [0x96A6EC, 0x5245A9], VERTICAL),
    preset("crimson", "Crimson", [0x4A0404, 0xCB1616], RISING),
    preset("sandstone", "Sandstone", [0xCDBE9B, 0x4F5E5A], DIAGONAL),
    preset("dusk", "Dusk", [0x6A59DB, 0xD24C8C], HORIZONTAL),
    preset("steel", "Steel", [0x42424B, 0xAFB1C2], HORIZONTAL),
    preset("azure", "Azure", [0x1252C4, 0x569BF3], HORIZONTAL),
    preset("mist", "Mist", [0x6D7DB2, 0xB6C3EF], HORIZONTAL),
    preset("graphite", "Graphite", [0x1D1D21, 0x4F4F50], HORIZONTAL),
    preset("indigo", "Indigo", [0x5F52DB, 0x383271], VERTICAL),
    preset("charcoal", "Charcoal", [0x444444, 0x232323], VERTICAL),
    preset("amber", "Amber", [0xF5AA19, 0xF37A16], VERTICAL),
    preset("forest", "Forest", [0x1A6440, 0x0F3F29], VERTICAL),
    preset("silver", "Silver", [0xABABAB, 0xEDEDED], VERTICAL),
    preset("glacier", "Glacier", [0x617EA9, 0xC0D9E7], VERTICAL),
    preset("ruby", "Ruby", [0xF55053, 0xC93335], VERTICAL),
    preset("gold", "Gold", [0xFFC009, 0xFFD42A], VERTICAL),
];

fn hex_color(hex: u32) -> BackgroundColor {
    let ch = |shift: u32| f64::from((hex >> shift) & 255) / 255.0;
    BackgroundColor::custom(ch(16), ch(8), ch(0))
}

impl GradientPreset {
    pub fn by_id(id: &str) -> Option<&'static GradientPreset> {
        PRESETS.iter().find(|p| p.id == id)
    }

    pub fn style(&self) -> BackgroundGradient {
        BackgroundGradient {
            id: self.id.into(),
            title: self.title.into(),
            colors: self.colors.iter().map(|&hex| hex_color(hex)).collect(),
            start: self.start,
            end: self.end,
        }
    }
}

type Premul = [f32; 4];

fn premul(c: &BackgroundColor) -> Premul {
    let a = c.alpha.clamp(0.0, 1.0) as f32;
    [
        c.red.clamp(0.0, 1.0) as f32 * a,
        c.green.clamp(0.0, 1.0) as f32 * a,
        c.blue.clamp(0.0, 1.0) as f32 * a,
        a,
    ]
}

fn even_stops(g: &BackgroundGradient) -> Vec<(f32, Premul)> {
    let last = g.colors.len().saturating_sub(1).max(1) as f32;
    g.colors
        .iter()
        .enumerate()
        .map(|(i, c)| (i as f32 / last, premul(c)))
        .collect()
}

fn sample(stops: &[(f32, Premul)], t: f32) -> Premul {
    let Some(&(first_at, first)) = stops.first() else {
        return [0.0; 4];
    };
    if t <= first_at {
        return first;
    }
    for pair in stops.windows(2) {
        let ((a_at, a), (b_at, b)) = (pair[0], pair[1]);
        if t <= b_at {
            let k = if b_at > a_at {
                (t - a_at) / (b_at - a_at)
            } else {
                1.0
            };
            return [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * k);
        }
    }
    stops[stops.len() - 1].1
}

pub fn fill_solid(target: &mut Pixmap, c: &BackgroundColor) {
    let ch = |v: f64| v.clamp(0.0, 1.0) as f32;
    let color = Color::from_rgba(ch(c.red), ch(c.green), ch(c.blue), ch(c.alpha));
    target.fill(color.unwrap_or(Color::BLACK));
}

/// Rendered per pixel on all cores: tiny-skia's gradient pipeline needs over 300 ms at 2560x1600.
pub fn draw_gradient(target: &mut Pixmap, gradient: &BackgroundGradient) {
    let (w, h) = (target.width() as usize, target.height() as usize);
    let stops = even_stops(gradient);
    if stops.is_empty() || w == 0 || h == 0 {
        return;
    }
    let (wf, hf) = (w as f32, h as f32);
    let start = (gradient.start.x as f32 * wf, gradient.start.y as f32 * hf);
    let axis = (
        gradient.end.x as f32 * wf - start.0,
        gradient.end.y as f32 * hf - start.1,
    );
    let len2 = axis.0 * axis.0 + axis.1 * axis.1;
    let constant_along_x = axis.0 == 0.0;
    target
        .data_mut()
        .par_chunks_exact_mut(w * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let py = y as f32 + 0.5;
            let row_base = (py - start.1) * axis.1;
            let mut cached = None;
            for (x, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let pxf = x as f32 + 0.5;
                let c = match cached {
                    Some(c) => c,
                    None => {
                        let t = if len2 > 0.0 {
                            (row_base + (pxf - start.0) * axis.0) / len2
                        } else {
                            0.0
                        };
                        let c = sample(&stops, t);
                        if constant_along_x {
                            cached = Some(c);
                        }
                        c
                    }
                };
                for (out, v) in px.iter_mut().zip(c) {
                    *out = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                }
            }
        });
}

pub fn draw_background(target: &mut Pixmap, style: &BackgroundStyle) {
    draw_background_with(target, style, None);
}

/// A wallpaper style without a decodable image falls back to the first bundled gradient.
pub fn draw_background_with(
    target: &mut Pixmap,
    style: &BackgroundStyle,
    wallpaper: Option<&Pixmap>,
) {
    match style {
        BackgroundStyle::Solid(c) => fill_solid(target, c),
        BackgroundStyle::Gradient(g) => draw_gradient(target, g),
        BackgroundStyle::Wallpaper { .. } => match wallpaper {
            Some(image) => draw_wallpaper(target, image),
            None => draw_gradient(target, &PRESETS[0].style()),
        },
        BackgroundStyle::None => {}
    }
}

pub fn render_gradient(gradient: &BackgroundGradient, width: u32, height: u32) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(width, height)?;
    draw_gradient(&mut pixmap, gradient);
    Some(pixmap)
}

pub fn render_background(style: &BackgroundStyle, width: u32, height: u32) -> Option<Pixmap> {
    let mut pixmap = Pixmap::new(width, height)?;
    draw_background(&mut pixmap, style);
    Some(pixmap)
}
