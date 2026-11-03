use es_ui::components::backgrounds::{dot_cells, dot_tile};
use gpui::{Bounds, Hsla, Pixels, RenderImage};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridKey {
    pub scale_factor: f32,
    pub workspace: Hsla,
    pub dot: Hsla,
}

/// One pre-rasterised tile of the workspace dot grid; thousands of quads a frame cost milliseconds.
pub fn render(key: GridKey) -> Option<Arc<RenderImage>> {
    dot_tile(key.scale_factor, Some(key.workspace), key.dot)
}

/// The window rectangles the grid tile is painted at to cover `bounds`.
pub fn cells(bounds: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
    dot_cells(bounds)
}
