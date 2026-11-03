use es_geom::Rect;
use image::RgbaImage;
use rxing::helpers::detect_multiple_in_luma;

pub fn decode_codes(image: &RgbaImage) -> Vec<String> {
    decode_code_boxes(image)
        .into_iter()
        .map(|(text, _)| text)
        .collect()
}

/// Each code with the box around its corner points, in image pixels, top to bottom.
pub fn decode_code_boxes(image: &RgbaImage) -> Vec<(String, Rect)> {
    let luma = image::imageops::grayscale(image);
    let (w, h) = luma.dimensions();
    let Ok(found) = detect_multiple_in_luma(luma.into_raw(), w, h) else {
        return Vec::new();
    };
    let mut payloads: Vec<(f32, f32, String, Rect)> = found
        .iter()
        .filter(|r| !r.getText().trim().is_empty())
        .map(|r| {
            let (x0, y0, x1, y1) = r.getPoints().iter().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
            );
            let rect = Rect::new(x0.into(), y0.into(), x1.into(), y1.into());
            (y0, x0, r.getText().to_owned(), rect)
        })
        .collect();
    payloads.sort_by(|a, b| {
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    payloads.dedup_by(|a, b| a.2 == b.2);
    payloads.into_iter().map(|(_, _, t, r)| (t, r)).collect()
}
