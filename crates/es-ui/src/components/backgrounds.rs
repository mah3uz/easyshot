use gpui::{
    AnyElement, App, Bounds, Div, Hsla, IntoElement, ParentElement, Pixels, Refineable, RenderOnce,
    StyleRefinement, Styled, Window, canvas, div, fill, point, px, size,
};

use crate::{convert::hsla, theme::tokens, tokens::LIGHT};

pub const DOT_SPACING: Pixels = px(16.);
pub const DOT_DIAMETER: Pixels = px(1.6);
pub const CHECKER_CELL: Pixels = px(10.);

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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let dot = t.text_tertiary;
        painted(
            &self.style,
            self.children,
            t.workspace,
            move |bounds, window| {
                let step = f32::from(DOT_SPACING);
                let d = DOT_DIAMETER;
                let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                let mut y = step / 2.;
                while y < h {
                    let mut x = step / 2.;
                    while x < w {
                        let origin = bounds.origin + point(px(x), px(y)) - point(d / 2., d / 2.);
                        window.paint_quad(
                            fill(Bounds::new(origin, size(d, d)), dot).corner_radii(d / 2.),
                        );
                        x += step;
                    }
                    y += step;
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
