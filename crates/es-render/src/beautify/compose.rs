use crate::beautify::camera::{self, Quality};
use crate::beautify::frame::{Frame, frame, paint_border, paint_image};
use crate::beautify::gradients::draw_background_with;
use crate::beautify::layout::{Layout, layout};
use crate::beautify::progressive_blur;
use crate::beautify::shadow::{paint_shadow, shadow_layer};
use crate::beautify::watermark;
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
    compose_with_wallpaper(image, annotations, settings, None)
}

pub fn compose_with_wallpaper(
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    wallpaper: Option<&Pixmap>,
) -> Option<Pixmap> {
    let lay = layout(
        f64::from(image.width()),
        f64::from(image.height()),
        settings,
    )
    .pixel_aligned();
    let (width, height) = lay.canvas_pixels();
    let mut canvas = Pixmap::new(width, height)?;
    draw_background_with(&mut canvas, &settings.style, wallpaper);

    let camera = settings.camera.has_effect();
    let mut staged = if camera {
        Some(Pixmap::new(width, height)?)
    } else {
        None
    };
    let target = staged.as_mut().unwrap_or(&mut canvas);
    paint_card(target, image, annotations, settings, &lay, camera);
    if let Some(layer) = staged {
        let layer = project_camera_layer(layer, &lay, settings);
        canvas.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    apply_progressive_blur(&mut canvas, settings, &lay);
    watermark::paint(&mut canvas, &settings.watermark);
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

fn project_camera_layer(layer: Pixmap, lay: &Layout, settings: &BackgroundSettings) -> Pixmap {
    camera::project(layer, lay, &settings.camera, Quality::Full)
}

fn apply_progressive_blur(canvas: &mut Pixmap, settings: &BackgroundSettings, lay: &Layout) {
    let mut image = lay.image;
    if settings.camera.has_effect() {
        let size = (f64::from(canvas.width()), f64::from(canvas.height()));
        if let Some(p) = camera::projection(lay.card, lay.image, size, &settings.camera) {
            let (xs, ys): (Vec<f64>, Vec<f64>) = p.quad.iter().copied().unzip();
            let fold =
                |v: &[f64], init: f64, f: fn(f64, f64) -> f64| v.iter().copied().fold(init, f);
            image = es_geom::Rect::new(
                fold(&xs, f64::INFINITY, f64::min),
                fold(&ys, f64::INFINITY, f64::min),
                fold(&xs, f64::NEG_INFINITY, f64::max),
                fold(&ys, f64::NEG_INFINITY, f64::max),
            );
        }
    }
    progressive_blur::apply_scoped(canvas, &settings.progressive_blur, image);
}
