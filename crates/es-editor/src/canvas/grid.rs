use super::underlay::render_image;
use es_ui::components::backgrounds::{DOT_DIAMETER, DOT_SPACING};
use gpui::{Bounds, Hsla, Pixels, RenderImage, point, px, size};
use std::sync::Arc;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Logical pixels per side of the repeated grid image; a whole number of dot steps.
pub const GRID_TILE: f32 = 512.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridKey {
    pub scale_factor: f32,
    pub workspace: Hsla,
    pub dot: Hsla,
}

fn color(c: Hsla) -> Color {
    let c = c.to_rgb();
    Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::TRANSPARENT)
}

/// One pre-rasterised tile of the workspace dot grid; thousands of quads a frame cost milliseconds.
pub fn render(key: GridKey) -> Option<Arc<RenderImage>> {
    let side = (GRID_TILE * key.scale_factor).round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(side, side)?;
    pixmap.fill(color(key.workspace));
    let step = f32::from(DOT_SPACING) * key.scale_factor;
    let r = f32::from(DOT_DIAMETER) * key.scale_factor / 2.0;
    let mut pb = PathBuilder::new();
    let mut y = step / 2.0;
    while y < side as f32 {
        let mut x = step / 2.0;
        while x < side as f32 {
            pb.push_circle(x, y, r);
            x += step;
        }
        y += step;
    }
    let path = pb.finish()?;
    let mut paint = Paint::default();
    paint.set_color(color(key.dot));
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    render_image(&pixmap)
}

/// The window rectangles the grid tile is painted at to cover `bounds`.
pub fn cells(bounds: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
    let mut out = Vec::new();
    let tile = px(GRID_TILE);
    let mut y = px(0.);
    while y < bounds.size.height {
        let mut x = px(0.);
        while x < bounds.size.width {
            out.push(Bounds::new(bounds.origin + point(x, y), size(tile, tile)));
            x += tile;
        }
        y += tile;
    }
    out
}
