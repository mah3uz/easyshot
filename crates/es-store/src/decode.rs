use image::{DynamicImage, ImageReader, ImageResult, Limits};
use std::io::Cursor;
use std::path::Path;

pub const MAX_SIDE: u32 = 32_768;
/// Covers two 8K screens side by side; the editor holds several copies of what it decodes.
pub const MAX_ALLOC: u64 = 256 << 20;

pub fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    limits
}

/// Every decode of an image the app did not just draw goes through here.
pub fn open_image(path: &Path) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(limits());
    reader.decode()
}

pub fn image_from_memory(bytes: &[u8]) -> ImageResult<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    reader.limits(limits());
    reader.decode()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbaImage::new(w, h)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
            }
        }
        !crc
    }

    /// A one-pixel PNG whose header claims the given size.
    fn claiming(w: u32, h: u32) -> Vec<u8> {
        let mut png = png(1, 1);
        png[16..20].copy_from_slice(&w.to_be_bytes());
        png[20..24].copy_from_slice(&h.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        png
    }

    #[test]
    fn an_image_past_the_side_limit_is_refused_from_its_header() {
        assert!(image_from_memory(&png(64, 64)).is_ok());
        assert!(matches!(
            image_from_memory(&claiming(1, MAX_SIDE + 1)),
            Err(image::ImageError::Limits(_))
        ));
    }

    #[test]
    fn an_image_within_the_sides_but_past_the_allocation_is_refused() {
        assert!(matches!(
            image_from_memory(&claiming(20_000, 20_000)),
            Err(image::ImageError::Limits(_))
        ));
    }
}
