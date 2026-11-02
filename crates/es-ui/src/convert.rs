use es_geom::{Point, Rect, Rgba};
use gpui::{Bounds, Hsla, Pixels, RenderImage, px};
use image::{Frame, RgbaImage};
use std::sync::Arc;
use tiny_skia::Pixmap;

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

/// Premultiplied RGBA into GPUI's straight-alpha BGRA.
pub fn render_image(pixmap: &Pixmap) -> Option<Arc<RenderImage>> {
    let mut data = pixmap.data().to_vec();
    for px in data.as_chunks_mut::<4>().0 {
        let a = px[3];
        let un = |c: u8| {
            if a == 0 {
                0
            } else {
                ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
            }
        };
        let (r, g, b) = (un(px[0]), un(px[1]), un(px[2]));
        px[0] = b;
        px[1] = g;
        px[2] = r;
    }
    let image = RgbaImage::from_raw(pixmap.width(), pixmap.height(), data)?;
    Some(Arc::new(RenderImage::new([Frame::new(image)])))
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
