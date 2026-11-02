use crate::beautify::wallpaper::draw_wallpaper;
use es_model::{BackgroundColor, BackgroundGradient, BackgroundStyle, UnitPoint};
use rayon::prelude::*;
use tiny_skia::{Color, Pixmap};

/// A white glow in unit coordinates; `extent` is the end radius in the glow's own elliptical space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
    pub extent: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientPreset {
    pub id: &'static str,
    pub title: &'static str,
    pub stops: [(u32, f32); 3],
    pub highlights: [Highlight; 2],
}

const fn h(x: f32, y: f32, opacity: f32, extent: f32) -> Highlight {
    Highlight {
        x,
        y,
        opacity,
        extent,
    }
}

pub const PRESETS: [GradientPreset; 10] = [
    GradientPreset {
        id: "soft-blush",
        title: "Blush / Lavender",
        stops: [(0xFAFAFA, 0.0), (0xF9E8F3, 0.48), (0xE7C8F1, 1.0)],
        highlights: [h(0.18, 0.08, 0.32, 0.35), h(0.86, 0.90, 0.16, 0.34)],
    },
    GradientPreset {
        id: "soft-peach",
        title: "Peach / Apricot",
        stops: [(0xFCFAF7, 0.0), (0xFBE6D8, 0.5), (0xFFB98D, 1.0)],
        highlights: [h(0.82, 0.08, 0.30, 0.34), h(0.14, 0.88, 0.18, 0.32)],
    },
    GradientPreset {
        id: "soft-mint",
        title: "Mint",
        stops: [(0xFBFCFA, 0.0), (0xE8F8F0, 0.48), (0xAEEACD, 1.0)],
        highlights: [h(0.18, 0.10, 0.34, 0.36), h(0.88, 0.86, 0.20, 0.34)],
    },
    GradientPreset {
        id: "soft-blue",
        title: "Powder Blue",
        stops: [(0xFBFCFD, 0.0), (0xE5F2FD, 0.5), (0xA8D7FF, 1.0)],
        highlights: [h(0.16, 0.08, 0.34, 0.35), h(0.88, 0.88, 0.22, 0.33)],
    },
    GradientPreset {
        id: "soft-butter",
        title: "Butter Yellow",
        stops: [(0xFDFCF7, 0.0), (0xFFF8D6, 0.48), (0xFFE677, 1.0)],
        highlights: [h(0.16, 0.08, 0.36, 0.36), h(0.88, 0.86, 0.22, 0.32)],
    },
    GradientPreset {
        id: "soft-lilac",
        title: "Lilac / Periwinkle",
        stops: [(0xFCFBFD, 0.0), (0xEEEAFE, 0.44), (0xAFAEFF, 1.0)],
        highlights: [h(0.18, 0.07, 0.34, 0.36), h(0.86, 0.84, 0.16, 0.34)],
    },
    GradientPreset {
        id: "soft-sage",
        title: "Sage Green",
        stops: [(0xFCFBF7, 0.0), (0xE8EDE1, 0.42), (0x91AD8A, 1.0)],
        highlights: [h(0.16, 0.08, 0.30, 0.36), h(0.88, 0.42, 0.20, 0.31)],
    },
    GradientPreset {
        id: "soft-coral",
        title: "Coral / Rose",
        stops: [(0xFCF9F7, 0.0), (0xF9D3CD, 0.46), (0xFF6F73, 1.0)],
        highlights: [h(0.18, 0.07, 0.30, 0.36), h(0.82, 0.70, 0.20, 0.31)],
    },
    GradientPreset {
        id: "soft-aqua",
        title: "Aqua / Cyan",
        stops: [(0xFBFDFD, 0.0), (0xDFF9FC, 0.48), (0x6FE2ED, 1.0)],
        highlights: [h(0.15, 0.08, 0.34, 0.36), h(0.88, 0.87, 0.24, 0.34)],
    },
    GradientPreset {
        id: "soft-mauve",
        title: "Dusty Mauve",
        stops: [(0xFBF8F6, 0.0), (0xE8DADB, 0.42), (0xB7949F, 1.0)],
        highlights: [h(0.17, 0.07, 0.30, 0.36), h(0.86, 0.38, 0.18, 0.32)],
    },
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
            colors: self.stops.iter().map(|&(hex, _)| hex_color(hex)).collect(),
            start: UnitPoint { x: 0.5, y: 0.0 },
            end: UnitPoint { x: 0.5, y: 1.0 },
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

/// Locations and highlights come from the bundled preset with the same id when the colour count
/// matches; any other gradient spreads its colours evenly and has no glows.
fn stops_and_highlights(g: &BackgroundGradient) -> (Vec<(f32, Premul)>, &'static [Highlight]) {
    let preset = GradientPreset::by_id(&g.id).filter(|p| p.stops.len() == g.colors.len());
    let last = g.colors.len().saturating_sub(1).max(1) as f32;
    let stops = g
        .colors
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let at = preset.map_or(i as f32 / last, |p| p.stops[i].1);
            (at, premul(c))
        })
        .collect();
    (stops, preset.map_or(&[], |p| &p.highlights[..]))
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
    let (stops, highlights) = stops_and_highlights(gradient);
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
    let glows: Vec<[f32; 5]> = highlights
        .iter()
        .rev()
        .filter_map(|g| {
            let rx = g.x.max(1.0 - g.x) * wf * std::f32::consts::SQRT_2;
            let ry = g.y.max(1.0 - g.y) * hf * std::f32::consts::SQRT_2;
            (rx > 0.0 && ry > 0.0 && g.extent > 0.0).then_some([
                g.x * wf,
                g.y * hf,
                1.0 / (rx * g.extent),
                1.0 / (ry * g.extent),
                g.opacity,
            ])
        })
        .collect();

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
                let mut c = match cached {
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
                for g in &glows {
                    let (dx, dy) = ((pxf - g[0]) * g[2], (py - g[1]) * g[3]);
                    let d2 = dx * dx + dy * dy;
                    if d2 < 1.0 {
                        let a = g[4] * (1.0 - d2.sqrt());
                        for ch in &mut c {
                            *ch = a + *ch * (1.0 - a);
                        }
                    }
                }
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
