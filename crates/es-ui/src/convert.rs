use es_geom::{Point, Rect, Rgba};
use gpui::{Bounds, Hsla, Pixels, px};

pub fn hsla(c: Rgba) -> Hsla {
    gpui::Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
    .into()
}

pub fn px_point(p: Point) -> gpui::Point<Pixels> {
    gpui::point(px(p.x as f32), px(p.y as f32))
}

pub fn bounds(r: Rect) -> Bounds<Pixels> {
    let r = r.abs();
    Bounds::new(
        px_point(r.origin()),
        gpui::size(px(r.width() as f32), px(r.height() as f32)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_keep_their_srgb_channels() {
        let c = hsla(Rgba::new(0.0, 0.5568628, 0.9843137, 0.25)).to_rgb();
        assert!((c.r - 0.0).abs() < 1e-5 && (c.g - 0.5568628).abs() < 1e-5);
        assert!((c.b - 0.9843137).abs() < 1e-5 && c.a == 0.25);
    }

    #[test]
    fn a_rect_drawn_backwards_is_the_same_bounds() {
        let forward = bounds(Rect::new(10.0, 20.0, 110.0, 70.0));
        let backward = bounds(Rect::new(110.0, 70.0, 10.0, 20.0));
        assert_eq!(forward, backward);
        assert_eq!(forward.origin, px_point(Point::new(10.0, 20.0)));
        assert_eq!(forward.size, gpui::size(px(100.0), px(50.0)));
    }
}
