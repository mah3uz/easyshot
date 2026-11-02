use es_model::BackgroundSettings;
use es_render::beautify::frame::{frame, paint_border};
use es_render::beautify::gradients::draw_background;
use es_render::beautify::layout;
use es_render::beautify::shadow::{paint_shadow, shadow_layer};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
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
        })
    }
}

/// Image pixels per page pixel for a display scale, in steps of √2 and never above 1.
pub fn stepped_scale(device_scale: f64, canvas_edge: f64) -> f64 {
    let stepped = 2f64.powf((device_scale.max(1e-3).log2() * 2.0).ceil() / 2.0);
    stepped.min(1.0).min(MAX_EDGE / canvas_edge.max(1.0))
}

/// Background, shadow and border, the layers that sit under the image card.
pub fn render(key: &UnderlayKey) -> Option<Pixmap> {
    let (w, h) = (
        f64::from(key.image.0) * key.scale,
        f64::from(key.image.1) * key.scale,
    );
    let lay = layout(w, h, &key.settings).pixel_aligned();
    let (cw, ch) = lay.canvas_pixels();
    let mut target = Pixmap::new(cw, ch)?;
    let s = &key.settings;
    draw_background(&mut target, &s.style);
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
    let mut data = pixmap.data().to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3];
        let un = |c: u8| {
            if a == 0 {
                0
            } else {
                ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
            }
        };
        let (r, g, b) = (un(px[0]), un(px[1]), un(px[2]));
        px[0] = b;
        px[1] = g;
        px[2] = r;
    }
    let image = RgbaImage::from_raw(pixmap.width(), pixmap.height(), data)?;
    Some(Arc::new(RenderImage::new([Frame::new(image)])))
}
