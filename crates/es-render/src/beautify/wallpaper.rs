use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

/// Scales to cover the target and centres, cropping the overflow; the aspect ratio is kept.
pub fn draw_wallpaper(target: &mut Pixmap, image: &Pixmap) {
    let (tw, th) = (target.width() as f32, target.height() as f32);
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    if tw <= 0.0 || th <= 0.0 || iw <= 0.0 || ih <= 0.0 {
        return;
    }
    let scale = (tw / iw).max(th / ih);
    let (dx, dy) = ((tw - iw * scale) / 2.0, (th - ih * scale) / 2.0);
    target.draw_pixmap(
        0,
        0,
        image.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..PixmapPaint::default()
        },
        Transform::from_row(scale, 0.0, 0.0, scale, dx, dy),
        None,
    );
}

const MAX_WALLPAPER_SIDE: u32 = 16_384;

/// Decodes PNG or JPEG bytes into premultiplied pixels; an oversized image reads as missing.
pub fn decode_wallpaper(bytes: &[u8]) -> Option<Pixmap> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_WALLPAPER_SIDE);
    limits.max_image_height = Some(MAX_WALLPAPER_SIDE);
    reader.limits(limits);
    let rgba = reader.decode().ok()?.into_rgba8();
    let mut pixmap = Pixmap::new(rgba.width(), rgba.height())?;
    for (dst, src) in pixmap.pixels_mut().iter_mut().zip(rgba.pixels()) {
        let [r, g, b, a] = src.0;
        *dst = tiny_skia::ColorU8::from_rgba(r, g, b, a).premultiply();
    }
    Some(pixmap)
}
