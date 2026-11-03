use image::{RgbaImage, imageops::FilterType};

/// Above this many pixels the editor works on a downscaled copy of the base image.
pub const AUTO_PIXELS: u64 = 40_000_000;
/// Either side above this cannot be uploaded as one texture on common GPUs.
pub const TEXTURE_LIMIT: u32 = 16384;
/// A preview stays at half the texture limit, so tiles upload with room to spare.
pub const PREVIEW_EDGE: u32 = 8192;
/// About 128 MB of pixels, which keeps the preview and the full image together well under a gigabyte.
pub const PREVIEW_PIXELS: u64 = 32_000_000;
const FORCED_DIVISOR: u32 = 2;

pub fn in_effect(width: u32, height: u32, forced: bool) -> bool {
    forced
        || u64::from(width) * u64::from(height) > AUTO_PIXELS
        || width.max(height) > TEXTURE_LIMIT
}

/// The size the preview is drawn at; the image's own size when nothing needs shrinking.
pub fn target(width: u32, height: u32, forced: bool) -> (u32, u32) {
    let (w, h) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let mut scale = if forced {
        1.0 / f64::from(FORCED_DIVISOR)
    } else {
        1.0
    };
    scale = scale.min(f64::from(PREVIEW_EDGE) / w.max(h));
    scale = scale.min((PREVIEW_PIXELS as f64 / (w * h)).sqrt());
    if scale >= 1.0 {
        return (width, height);
    }
    (
        ((w * scale) as u32).clamp(1, width.max(1)),
        ((h * scale) as u32).clamp(1, height.max(1)),
    )
}

/// `None` when the image is already small enough to show as it is.
pub fn downscale(base: &RgbaImage, forced: bool) -> Option<RgbaImage> {
    let (w, h) = target(base.width(), base.height(), forced);
    if (w, h) == base.dimensions() || base.width() == 0 || base.height() == 0 {
        return None;
    }
    Some(image::imageops::resize(base, w, h, FilterType::Triangle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capture_over_40_megapixels_or_past_the_texture_limit_is_shrunk_to_the_cap() {
        for (w, h) in [(9000, 6000), (20000, 500), (500, 20000)] {
            assert!(in_effect(w, h, false), "{w}x{h}");
            let (tw, th) = target(w, h, false);
            assert!(tw.max(th) <= PREVIEW_EDGE, "{tw}x{th}");
            assert!(u64::from(tw) * u64::from(th) <= PREVIEW_PIXELS, "{tw}x{th}");
        }
    }

    #[test]
    fn an_ordinary_capture_is_left_alone_unless_the_preview_is_forced() {
        assert!(!in_effect(3840, 2160, false));
        assert_eq!(target(3840, 2160, false), (3840, 2160));
        assert!(in_effect(3840, 2160, true));
        assert_eq!(target(3840, 2160, true), (1920, 1080));
    }

    #[test]
    fn the_preview_keeps_the_aspect_ratio() {
        assert_eq!(target(12000, 3000, false), (8192, 2048));
    }
}
