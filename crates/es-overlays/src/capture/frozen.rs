use es_geom::Rect;
use es_model::platform::Frame;
use gpui::RenderImage;
use std::sync::Arc;

// GPU textures commonly stop at 4096 px a side, and a frozen 5K or 8K output is larger.
pub const MAX_TILE: u32 = 4096;

#[derive(Clone)]
pub struct Tile {
    /// Logical, relative to the output's top-left.
    pub local: Rect,
    pub image: Arc<RenderImage>,
}

pub fn tiles(frame: &Frame) -> Vec<Tile> {
    let (w, h) = frame.size_px;
    let stride = w as usize * 4;
    if w == 0 || h == 0 || frame.bgra.len() < stride * h as usize {
        return Vec::new();
    }
    let (sx, sy) = (
        frame.logical.width() / f64::from(w),
        frame.logical.height() / f64::from(h),
    );
    let mut out = Vec::new();
    for ty in (0..h).step_by(MAX_TILE as usize) {
        for tx in (0..w).step_by(MAX_TILE as usize) {
            let (tw, th) = (MAX_TILE.min(w - tx), MAX_TILE.min(h - ty));
            let mut bgra = Vec::with_capacity(tw as usize * th as usize * 4);
            for y in ty..ty + th {
                let start = y as usize * stride + tx as usize * 4;
                bgra.extend_from_slice(&frame.bgra[start..start + tw as usize * 4]);
            }
            let Some(buffer) = image::RgbaImage::from_raw(tw, th, bgra) else {
                continue;
            };
            out.push(Tile {
                local: Rect::new(
                    f64::from(tx) * sx,
                    f64::from(ty) * sy,
                    f64::from(tx + tw) * sx,
                    f64::from(ty + th) * sy,
                ),
                image: Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])),
            });
        }
    }
    out
}
