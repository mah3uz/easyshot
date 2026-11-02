use crate::beautify::frame::{Frame, frame, paint_border, paint_image};
use crate::beautify::gradients::draw_background;
use crate::beautify::layout::{Layout, layout};
use crate::beautify::shadow::{paint_shadow, shadow_layer};
use crate::paint::paint_draw_list;
use es_anno::draw::{DrawItem, DrawList};
use es_geom::Affine;
use es_model::BackgroundSettings;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

/// Draw order: background, shadow and border backing, image clipped to its rounded corners,
/// annotations, camera projection, scene blur, watermark.
pub fn compose(
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
) -> Option<Pixmap> {
    let lay = layout(
        f64::from(image.width()),
        f64::from(image.height()),
        settings,
    )
    .pixel_aligned();
    let (width, height) = lay.canvas_pixels();
    let mut canvas = Pixmap::new(width, height)?;
    draw_background(&mut canvas, &settings.style);

    let camera = settings.camera.has_effect();
    let mut staged = if camera {
        Some(Pixmap::new(width, height)?)
    } else {
        None
    };
    let target = staged.as_mut().unwrap_or(&mut canvas);
    paint_card(target, image, annotations, settings, &lay, camera);
    if let Some(layer) = staged {
        let layer = project_camera_layer(layer, settings);
        canvas.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    apply_progressive_blur(&mut canvas, settings);
    paint_watermark(&mut canvas, settings);
    Some(canvas)
}

fn paint_card(
    target: &mut Pixmap,
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    lay: &Layout,
    always_shadow: bool,
) {
    let frame = frame(lay);
    if settings.is_enabled() || always_shadow {
        let edge = lay.card.width().min(lay.card.height());
        if let Some(layer) = shadow_layer(settings.shadow_style, settings.shadow, edge) {
            paint_shadow(target, &frame.card, &layer);
        }
    }
    if lay.border_width > 0.0 {
        paint_border(target, &frame, &settings.border);
    }
    paint_image_clipped(target, image, &frame);

    if annotations.items.is_empty() {
        return;
    }
    let origin = Affine::translate((lay.image.x0, lay.image.y0));
    let snapshot = if has_redaction(&annotations.items) {
        let mut s = Pixmap::new(target.width(), target.height());
        if let Some(s) = s.as_mut() {
            s.draw_pixmap(
                lay.image.x0 as i32,
                lay.image.y0 as i32,
                image.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        s
    } else {
        None
    };
    let snapshot = snapshot.as_ref().unwrap_or(image);
    paint_draw_list(annotations, target, origin, frame.image, snapshot);
}

fn paint_image_clipped(target: &mut Pixmap, image: &Pixmap, frame: &Frame) {
    if frame.image.radii.iter().all(|r| *r <= 0.0) {
        target.draw_pixmap(
            frame.image.rect.x0 as i32,
            frame.image.rect.y0 as i32,
            image.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    } else {
        paint_image(target, image, frame);
    }
}

fn has_redaction(items: &[DrawItem]) -> bool {
    items.iter().any(|item| match item {
        DrawItem::Redaction { .. } => true,
        DrawItem::Group { items, .. } => has_redaction(items),
        _ => false,
    })
}

fn project_camera_layer(layer: Pixmap, _settings: &BackgroundSettings) -> Pixmap {
    layer
}

fn apply_progressive_blur(_canvas: &mut Pixmap, _settings: &BackgroundSettings) {}

fn paint_watermark(_canvas: &mut Pixmap, _settings: &BackgroundSettings) {}
