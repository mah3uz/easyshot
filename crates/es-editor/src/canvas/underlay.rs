use crate::inspector::background::wallpaper_cache::wallpaper_bytes;
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::frame::{frame, paint_border};
use es_render::beautify::gradients::draw_background_with;
use es_render::beautify::layout;
use es_render::beautify::shadow::{paint_shadow, shadow_layer};
use es_render::beautify::wallpaper::decode_wallpaper;
use es_render::beautify::wallpaper_cache::WallpaperCache;
use gpui::RenderImage;
use std::sync::Arc;
use tiny_skia::Pixmap;

/// Above this the background is upscaled rather than rendered larger; it has no fine detail.
pub const MAX_EDGE: f64 = 4096.0;

#[derive(Debug, Clone, PartialEq)]
pub struct UnderlayKey {
    pub settings: BackgroundSettings,
    pub image: (u32, u32),
    /// Underlay pixels per page pixel, in steps of √2 and never above 1.
    pub scale: f64,
    pub morph: Option<Morph>,
}

/// A change of look part of the way along: layout and shadow come from between the two looks.
#[derive(Debug, Clone, PartialEq)]
pub struct Morph {
    pub from: BackgroundSettings,
    pub t: f64,
}

impl UnderlayKey {
    /// `None` when the look draws nothing around the image.
    pub fn new(
        settings: &BackgroundSettings,
        image: (u32, u32),
        canvas_edge: f64,
        device_scale: f64,
    ) -> Option<Self> {
        if !settings.requires_canvas_layout() || image.0 == 0 || image.1 == 0 {
            return None;
        }
        Some(Self {
            settings: settings.clone(),
            image,
            scale: stepped_scale(device_scale, canvas_edge),
            morph: None,
        })
    }

    pub fn morphing(mut self, morph: Option<Morph>) -> Self {
        self.morph = morph;
        self
    }
}

/// Image pixels per page pixel for a display scale, in steps of √2 and never above 1.
pub fn stepped_scale(device_scale: f64, canvas_edge: f64) -> f64 {
    let stepped = 2f64.powf((device_scale.max(1e-3).log2() * 2.0).ceil() / 2.0);
    stepped.min(1.0).min(MAX_EDGE / canvas_edge.max(1.0))
}

/// The wallpaper a style names, decoded once and kept in a size that covers `canvas`.
pub fn cached_wallpaper(
    cache: &WallpaperCache,
    style: &BackgroundStyle,
    canvas: (u32, u32),
) -> Option<Arc<Pixmap>> {
    let BackgroundStyle::Wallpaper { path } = style else {
        return None;
    };
    cache.get(path, canvas, || {
        wallpaper_bytes(path).and_then(|b| decode_wallpaper(&b))
    })
}

/// Background, shadow and border, the layers that sit under the image card.
pub fn render(key: &UnderlayKey, wallpapers: &WallpaperCache) -> Option<Pixmap> {
    let (w, h) = (
        f64::from(key.image.0) * key.scale,
        f64::from(key.image.1) * key.scale,
    );
    let mut lay = layout(w, h, &key.settings).pixel_aligned();
    let mut settings = key.settings.clone();
    if let Some(m) = &key.morph {
        lay = layout(w, h, &m.from).pixel_aligned().lerp(&lay, m.t);
        let from_shadow = if m.from.is_enabled() || m.from.camera.has_effect() {
            m.from.shadow
        } else {
            0.0
        };
        settings.shadow = from_shadow + (settings.shadow - from_shadow) * m.t;
    }
    let (cw, ch) = lay.canvas_pixels();
    let mut target = Pixmap::new(cw, ch)?;
    let s = &settings;
    let wallpaper = cached_wallpaper(wallpapers, &s.style, (cw, ch));
    draw_background_with(&mut target, &s.style, wallpaper.as_deref());
    let f = frame(&lay);
    if s.is_enabled() || s.camera.has_effect() {
        let edge = lay.card.width().min(lay.card.height());
        if let Some(layer) = shadow_layer(s.shadow_style, s.shadow, edge) {
            paint_shadow(&mut target, &f.card, &layer);
        }
    }
    if lay.border_width > 0.0 {
        paint_border(&mut target, &f, &s.border);
    }
    Some(target)
}

/// Premultiplied RGBA into GPUI's straight-alpha BGRA.
pub fn render_image(pixmap: &Pixmap) -> Option<Arc<RenderImage>> {
    es_ui::convert::render_image(pixmap)
}
