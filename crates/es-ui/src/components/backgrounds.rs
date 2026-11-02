use gpui::{
    AnyElement, App, Bounds, Corners, Div, Global, Hsla, IntoElement, ParentElement, Pixels,
    Refineable, RenderImage, RenderOnce, StyleRefinement, Styled, Window, canvas, div, fill, point,
    px, size,
};
use std::sync::Arc;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

use crate::{
    convert::{hsla, render_image},
    theme::tokens,
    tokens::LIGHT,
};

pub const DOT_SPACING: Pixels = px(16.);
pub const DOT_DIAMETER: Pixels = px(1.6);
pub const CHECKER_CELL: Pixels = px(10.);
/// Logical pixels per side of the repeated dot image; a whole number of dot steps.
pub const DOT_TILE: f32 = 512.0;

fn skia_color(c: Hsla) -> Color {
    let c = c.to_rgb();
    Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::TRANSPARENT)
}

/// One tile of the dot grid, filled with `base` when given and transparent otherwise.
pub fn dot_tile(scale_factor: f32, base: Option<Hsla>, dot: Hsla) -> Option<Arc<RenderImage>> {
    let side = (DOT_TILE * scale_factor).round().max(1.0) as u32;
    let mut pixmap = Pixmap::new(side, side)?;
    if let Some(base) = base {
        pixmap.fill(skia_color(base));
    }
    let step = f32::from(DOT_SPACING) * scale_factor;
    let r = f32::from(DOT_DIAMETER) * scale_factor / 2.0;
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
    paint.set_color(skia_color(dot));
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

/// The rectangles a dot tile is painted at to cover `bounds`.
pub fn dot_cells(bounds: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
    let mut out = Vec::new();
    let tile = px(DOT_TILE);
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

/// The tile the last dot grid was drawn with, so a frame paints an image instead of thousands of quads.
#[derive(Default)]
struct DotTile(Option<((u32, Hsla), Arc<RenderImage>)>);

impl Global for DotTile {}

fn cached_dot_tile(window: &mut Window, cx: &mut App, dot: Hsla) -> Option<Arc<RenderImage>> {
    let key = (window.scale_factor().to_bits(), dot);
    let cache = cx.default_global::<DotTile>();
    if let Some((k, image)) = &cache.0
        && *k == key
    {
        return Some(image.clone());
    }
    let image = dot_tile(window.scale_factor(), None, dot)?;
    if let Some((_, old)) = cache.0.replace((key, image.clone())) {
        let _ = window.drop_image(old);
    }
    Some(image)
}

fn painted(
    style: &StyleRefinement,
    children: Vec<AnyElement>,
    base: Hsla,
    paint: impl FnOnce(Bounds<Pixels>, &mut Window) + 'static,
) -> Div {
    let mut root = div();
    root.style().refine(style);
    root.relative()
        .overflow_hidden()
        .bg(base)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| paint(bounds, window),
            )
            .absolute()
            .size_full(),
        )
        .children(children)
}

#[derive(IntoElement, Default)]
pub struct DotGridBackground {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl DotGridBackground {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for DotGridBackground {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for DotGridBackground {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for DotGridBackground {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (workspace, dot) = {
            let t = tokens(cx);
            (t.workspace, t.text_tertiary)
        };
        let tile = cached_dot_tile(window, cx, dot);
        painted(
            &self.style,
            self.children,
            workspace,
            move |bounds, window| {
                let Some(tile) = tile else {
                    return;
                };
                for at in dot_cells(bounds) {
                    let _ =
                        window.paint_image(bounds, at, Corners::default(), tile.clone(), 0, false);
                }
            },
        )
    }
}

/// Shows where an image is transparent; it looks the same in both appearances.
#[derive(IntoElement, Default)]
pub struct CheckerBackground {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl CheckerBackground {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Styled for CheckerBackground {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for CheckerBackground {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for CheckerBackground {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let light = hsla(LIGHT.window);
        let dark = hsla(LIGHT.track);
        painted(&self.style, self.children, light, move |bounds, window| {
            let cell = f32::from(CHECKER_CELL);
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let (cols, rows) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
            for row in 0..rows {
                for col in (0..cols).filter(|col| (row + col) % 2 == 1) {
                    let x = col as f32 * cell;
                    let y = row as f32 * cell;
                    let square = Bounds::new(
                        bounds.origin + point(px(x), px(y)),
                        size(px(cell.min(w - x)), px(cell.min(h - y))),
                    );
                    window.paint_quad(fill(square, dark));
                }
            }
        })
    }
}
