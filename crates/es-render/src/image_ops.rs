use image::{RgbaImage, imageops};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTransform {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}

impl ImageTransform {
    pub fn apply(self, img: &RgbaImage) -> RgbaImage {
        match self {
            Self::RotateLeft => imageops::rotate270(img),
            Self::RotateRight => imageops::rotate90(img),
            Self::FlipHorizontal => imageops::flip_horizontal(img),
            Self::FlipVertical => imageops::flip_vertical(img),
        }
    }

    pub fn output_size(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::RotateLeft | Self::RotateRight => (height, width),
            Self::FlipHorizontal | Self::FlipVertical => (width, height),
        }
    }
}

/// The rect is clamped to the image; `None` when nothing is left.
pub fn crop(img: &RgbaImage, x: i64, y: i64, w: i64, h: i64) -> Option<RgbaImage> {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let x0 = x.clamp(0, iw);
    let y0 = y.clamp(0, ih);
    let x1 = x.saturating_add(w).clamp(0, iw);
    let y1 = y.saturating_add(h).clamp(0, ih);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let view = imageops::crop_imm(
        img,
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
    );
    Some(view.to_image())
}
