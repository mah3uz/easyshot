use crate::text::skia_path;
use es_model::WatermarkSettings;
use es_text::{FontFamily, TextEngine, TextStyle};
use tiny_skia::{Color, FillRule, Paint, Pixmap, Transform};

const MAX_OPACITY: f64 = 0.75;

/// Rows tile the canvas, rotated about its centre; the grid covers the diagonal so no corner is bare.
pub fn paint(canvas: &mut Pixmap, settings: &WatermarkSettings) {
    let text = settings.text.trim();
    let (w, h) = (f64::from(canvas.width()), f64::from(canvas.height()));
    if !settings.is_visible() || w <= 1.0 || h <= 1.0 {
        return;
    }
    let style = TextStyle {
        family: FontFamily::Compact,
        size: settings.font_size.clamp(1.0, 1000.0),
        bold: true,
        ..TextStyle::default()
    };
    let mut engine = TextEngine::bundled();
    let layout = engine.measure(text, &style, None);
    let (tw, th) = (layout.size.width, layout.size.height);
    if tw <= 0.0 || th <= 0.0 {
        return;
    }
    let Some(glyphs) = skia_path(&engine.outline(&layout).path) else {
        return;
    };

    let rows = settings.density.round().max(2.0);
    let spacing_y = h / rows;
    let spacing_x = (spacing_y * 2.2).max(1.0);
    let diagonal = w.hypot(h);
    let columns = (diagonal / spacing_x).ceil().max(2.0) as i64;
    let row_count = (diagonal / spacing_y).ceil().max(2.0) as i64;
    let origin_x = w / 2.0 - columns as f64 * spacing_x / 2.0;
    let origin_y = h / 2.0 - row_count as f64 * spacing_y / 2.0;

    let c = settings.color;
    let alpha = settings.opacity.clamp(0.0, MAX_OPACITY) as f32;
    let Some(color) = Color::from_rgba(
        c.r.clamp(0.0, 1.0),
        c.g.clamp(0.0, 1.0),
        c.b.clamp(0.0, 1.0),
        alpha * c.a.clamp(0.0, 1.0),
    ) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let rotate = Transform::from_rotate_at(
        settings.rotation_degrees as f32,
        (w / 2.0) as f32,
        (h / 2.0) as f32,
    );
    for row in 0..=row_count {
        for column in 0..=columns {
            let cx = origin_x + column as f64 * spacing_x;
            let cy = origin_y + row as f64 * spacing_y;
            let ts = rotate.pre_translate((cx - tw / 2.0) as f32, (cy - th / 2.0) as f32);
            canvas.fill_path(&glyphs, &paint, FillRule::Winding, ts, None);
        }
    }
}
