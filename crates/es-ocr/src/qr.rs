use image::RgbaImage;
use rxing::helpers::detect_multiple_in_luma;

pub fn decode_codes(image: &RgbaImage) -> Vec<String> {
    let luma = image::imageops::grayscale(image);
    let (w, h) = luma.dimensions();
    let Ok(found) = detect_multiple_in_luma(luma.into_raw(), w, h) else {
        return Vec::new();
    };
    let mut payloads: Vec<(f32, f32, String)> = found
        .iter()
        .filter(|r| !r.getText().trim().is_empty())
        .map(|r| {
            let (x, y) = r
                .getPoints()
                .iter()
                .fold((f32::MAX, f32::MAX), |(mx, my), p| {
                    (mx.min(p.x), my.min(p.y))
                });
            (y, x, r.getText().to_owned())
        })
        .collect();
    payloads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    payloads.dedup_by(|a, b| a.2 == b.2);
    payloads.into_iter().map(|(_, _, t)| t).collect()
}
