use es_geom::Rect;
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::sync::Arc;

/// Kept well under the GPU atlas's texture cap.
pub const TILE: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl TileRect {
    pub fn page_rect(&self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.x + self.w),
            f64::from(self.y + self.h),
        )
    }
}

pub fn grid(width: u32, height: u32, tile: u32) -> Vec<TileRect> {
    let tile = tile.max(1);
    let mut out = Vec::new();
    let mut y = 0;
    while y < height {
        let mut x = 0;
        while x < width {
            out.push(TileRect {
                x,
                y,
                w: tile.min(width - x),
                h: tile.min(height - y),
            });
            x += tile;
        }
        y += tile;
    }
    out
}

/// Indices of the tiles that overlap `visible`, given in page pixels.
pub fn visible(tiles: &[TileRect], visible: Rect) -> Vec<usize> {
    tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            let r = t.page_rect();
            r.x0 < visible.x1 && r.x1 > visible.x0 && r.y0 < visible.y1 && r.y1 > visible.y0
        })
        .map(|(i, _)| i)
        .collect()
}

/// GPUI images are straight-alpha BGRA.
pub fn render_image(source: &RgbaImage, r: TileRect) -> Arc<RenderImage> {
    let mut out = RgbaImage::new(r.w, r.h);
    let stride = source.width() as usize * 4;
    let raw = source.as_raw();
    let dst: &mut [u8] = out.as_mut();
    for row in 0..r.h as usize {
        let s = (r.y as usize + row) * stride + r.x as usize * 4;
        let d = row * r.w as usize * 4;
        let src = &raw[s..s + r.w as usize * 4];
        let out_row = &mut dst[d..d + r.w as usize * 4];
        for (o, i) in out_row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(src.as_chunks::<4>().0)
        {
            o.copy_from_slice(&[i[2], i[1], i[0], i[3]]);
        }
    }
    Arc::new(RenderImage::new([Frame::new(out)]))
}

pub struct BaseTiles {
    revision: u64,
    rects: Vec<TileRect>,
    images: Vec<Option<Arc<RenderImage>>>,
}

impl BaseTiles {
    pub fn new(revision: u64, width: u32, height: u32) -> Self {
        let rects = grid(width, height, TILE);
        let images = vec![None; rects.len()];
        Self {
            revision,
            rects,
            images,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn rects(&self) -> &[TileRect] {
        &self.rects
    }

    /// Uploads lazily, so a tall scroll capture only converts what is seen.
    pub fn image(&mut self, index: usize, source: &RgbaImage) -> Option<Arc<RenderImage>> {
        let rect = *self.rects.get(index)?;
        let slot = self.images.get_mut(index)?;
        Some(
            slot.get_or_insert_with(|| render_image(source, rect))
                .clone(),
        )
    }

    /// Hands back one tile's image; it is built again the next time the tile is visible.
    pub fn release(&mut self, index: usize) -> Option<Arc<RenderImage>> {
        self.images.get_mut(index)?.take()
    }

    pub fn into_images(self) -> impl Iterator<Item = Arc<RenderImage>> {
        self.images.into_iter().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_covers_the_image_exactly_once() {
        let tiles = grid(9000, 300, TILE);
        assert_eq!(tiles.len(), 3);
        assert_eq!(tiles.iter().map(|t| t.w * t.h).sum::<u32>(), 9000 * 300);
        assert_eq!(tiles[2].w, 9000 - 2 * TILE);
    }

    #[test]
    fn a_tile_is_visible_only_when_it_overlaps_the_view() {
        let tiles = grid(9000, 300, TILE);
        assert_eq!(visible(&tiles, Rect::new(0.0, 0.0, 100.0, 100.0)), [0]);
        assert_eq!(
            visible(&tiles, Rect::new(4000.0, 0.0, 4200.0, 10.0)),
            [0, 1]
        );
        assert!(visible(&tiles, Rect::new(0.0, 400.0, 100.0, 500.0)).is_empty());
    }
}
