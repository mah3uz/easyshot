use crate::image_ops::resize_premultiplied;
use tiny_skia::Pixmap;

/// Scales to cover the target and centres, cropping the overflow; the aspect ratio is kept.
pub fn draw_wallpaper(target: &mut Pixmap, image: &Pixmap) {
    let (tw, th) = (target.width(), target.height());
    let (iw, ih) = (image.width(), image.height());
    if tw == 0 || th == 0 || iw == 0 || ih == 0 {
        return;
    }
    let scale = (f64::from(tw) / f64::from(iw)).max(f64::from(th) / f64::from(ih));
    let (cw, ch) = (f64::from(tw) / scale, f64::from(th) / scale);
    let crop = (
        (f64::from(iw) - cw) / 2.0,
        (f64::from(ih) - ch) / 2.0,
        cw,
        ch,
    );
    if resize_premultiplied(image, target, Some(crop)).is_none() {
        tracing::warn!("the wallpaper could not be scaled");
    }
}

/// The picture scaled by `factor` (at most 1), for keeping a smaller copy of a large wallpaper.
pub fn shrink(image: &Pixmap, factor: f64) -> Option<Pixmap> {
    let w = ((f64::from(image.width()) * factor).round() as u32).max(1);
    let h = ((f64::from(image.height()) * factor).round() as u32).max(1);
    let mut out = Pixmap::new(w, h)?;
    resize_premultiplied(image, &mut out, None)?;
    Some(out)
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
