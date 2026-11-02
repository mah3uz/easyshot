use crate::beautify::blit;
use crate::beautify::camera::{self, Quality};
use crate::beautify::frame::{Frame, frame, paint_border, rounded_path};
use crate::beautify::gradients::draw_background_with;
use crate::beautify::layout::{Layout, layout};
use crate::beautify::progressive_blur;
use crate::beautify::shadow::{paint_shadow, shadow_layer};
use crate::beautify::watermark;
use crate::paint::{ClipShape, paint_draw_list};
use es_anno::draw::{DrawItem, DrawList};
use es_geom::{Affine, Rect};
use es_model::{BackgroundSettings, WatermarkSettings};
use tiny_skia::{FillRule, Mask, Pixmap, Transform};

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
    compose_scaled(image, annotations, settings, wallpaper, (1.0, 1.0))
}

/// `image` is the screenshot already resized by `page_scale`, which maps annotation page space onto it.
pub fn compose_scaled(
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    wallpaper: Option<&Pixmap>,
    page_scale: (f64, f64),
) -> Option<Pixmap> {
    let lay = image_layout(image, settings);
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
    paint_card(
        target,
        image,
        annotations,
        settings,
        &lay,
        camera,
        page_scale,
    );
    if let Some(layer) = staged {
        let layer = project_camera_layer(layer, &lay, settings);
        blit::over(&mut canvas, &layer, 0, 0, None);
    }

    let scope = blur_scope(&canvas, &lay, settings);
    progressive_blur::apply_scoped(&mut canvas, &settings.progressive_blur, scope);
    paint_watermark(&mut canvas, &settings.watermark, page_scale.0);
    Some(canvas)
}

/// The size of the canvas `compose_scaled` and `stage` make around an image of this size.
pub fn canvas_size(image: (u32, u32), settings: &BackgroundSettings) -> (u32, u32) {
    layout(f64::from(image.0), f64::from(image.1), settings)
        .pixel_aligned()
        .canvas_pixels()
}

fn image_layout(image: &Pixmap, settings: &BackgroundSettings) -> Layout {
    layout(
        f64::from(image.width()),
        f64::from(image.height()),
        settings,
    )
    .pixel_aligned()
}

/// The pieces before the camera and the blur, for previews that project and blur them many times.
pub struct Stage {
    pub layout: Layout,
    pub background: Pixmap,
    /// Shadow, border, image and annotations on a transparent canvas-sized layer.
    pub card: Pixmap,
}

pub fn stage(
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    wallpaper: Option<&Pixmap>,
    page_scale: (f64, f64),
) -> Option<Stage> {
    let lay = image_layout(image, settings);
    let (width, height) = lay.canvas_pixels();
    let mut background = Pixmap::new(width, height)?;
    draw_background_with(&mut background, &settings.style, wallpaper);
    let mut card = Pixmap::new(width, height)?;
    let camera = settings.camera.has_effect();
    paint_card(
        &mut card,
        image,
        annotations,
        settings,
        &lay,
        camera,
        page_scale,
    );
    Some(Stage {
        layout: lay,
        background,
        card,
    })
}

/// The font size is in output pixels, so a scaled render scales it too.
pub fn paint_watermark(canvas: &mut Pixmap, mark: &WatermarkSettings, scale: f64) {
    let mut mark = mark.clone();
    mark.font_size *= scale;
    watermark::paint(canvas, &mark);
}

fn paint_card(
    target: &mut Pixmap,
    image: &Pixmap,
    annotations: &DrawList,
    settings: &BackgroundSettings,
    lay: &Layout,
    always_shadow: bool,
    (sx, sy): (f64, f64),
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
    let origin =
        Affine::translate((lay.image.x0, lay.image.y0)) * Affine::scale_non_uniform(sx, sy);
    let clip = ClipShape {
        rect: Rect::new(0.0, 0.0, lay.image.width() / sx, lay.image.height() / sy),
        radii: lay.image_radii.map(|r| r / sx),
    };
    let snapshot = if has_redaction(&annotations.items) {
        let mut s = Pixmap::new(target.width(), target.height());
        if let Some(s) = s.as_mut() {
            blit::over(s, image, lay.image.x0 as i32, lay.image.y0 as i32, None);
        }
        s
    } else {
        None
    };
    let snapshot = snapshot.as_ref().unwrap_or(image);
    paint_draw_list(annotations, target, origin, clip, snapshot);
}

fn paint_image_clipped(target: &mut Pixmap, image: &Pixmap, frame: &Frame) {
    let (x, y) = (frame.image.rect.x0 as i32, frame.image.rect.y0 as i32);
    if frame.image.radii.iter().all(|r| *r <= 0.0) {
        return blit::over(target, image, x, y, None);
    }
    let (Some(mut corners), Some(path)) = (
        Mask::new(image.width(), image.height()),
        rounded_path(frame.image.rect, frame.image.radii),
    ) else {
        return;
    };
    let to_image = Transform::from_translate(-x as f32, -y as f32);
    corners.fill_path(&path, FillRule::Winding, true, to_image);
    blit::over(target, image, x, y, Some(&corners));
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

/// Where a clipped blur applies: the image, or the bounds of its projection under the camera.
pub fn blur_scope(canvas: &Pixmap, lay: &Layout, settings: &BackgroundSettings) -> Rect {
    let mut image = lay.image;
    if settings.camera.has_effect() {
        let size = (f64::from(canvas.width()), f64::from(canvas.height()));
        if let Some(p) = camera::projection(lay.card, lay.image, size, &settings.camera) {
            let (xs, ys): (Vec<f64>, Vec<f64>) = p.quad.iter().copied().unzip();
            let fold =
                |v: &[f64], init: f64, f: fn(f64, f64) -> f64| v.iter().copied().fold(init, f);
            image = Rect::new(
                fold(&xs, f64::INFINITY, f64::min),
                fold(&ys, f64::INFINITY, f64::min),
                fold(&xs, f64::NEG_INFINITY, f64::max),
                fold(&ys, f64::NEG_INFINITY, f64::max),
            );
        }
    }
    image
}
