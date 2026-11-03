use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;

pub(super) const MAX_EDGE: u32 = 1280;
const MAX_PNG_BYTES: usize = 1 << 20;
const JPEG_QUALITY: u8 = 85;

pub(super) struct Preview {
    pub data: String,
    pub mime: &'static str,
    pub width: u32,
    pub height: u32,
}

/// The file is read and shrunk on a blocking thread: a 4K screenshot takes real CPU.
pub(super) async fn preview(path: String) -> Result<Preview, String> {
    tokio::task::spawn_blocking(move || shrink(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

fn shrink(path: &Path) -> Result<Preview, String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let img = if img.width().max(img.height()) > MAX_EDGE {
        img.resize(MAX_EDGE, MAX_EDGE, FilterType::Triangle)
    } else {
        img
    };
    let (width, height) = (img.width(), img.height());
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    if png.len() <= MAX_PNG_BYTES {
        return Ok(Preview {
            data: STANDARD.encode(&png),
            mime: "image/png",
            width,
            height,
        });
    }
    let mut jpeg = Vec::new();
    let rgb = DynamicImage::ImageRgb8(img.to_rgb8());
    rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut jpeg,
        JPEG_QUALITY,
    ))
    .map_err(|e| e.to_string())?;
    Ok(Preview {
        data: STANDARD.encode(&jpeg),
        mime: "image/jpeg",
        width,
        height,
    })
}
