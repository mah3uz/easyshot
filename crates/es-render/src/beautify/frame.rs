use crate::beautify::layout::Layout;
use crate::paint::ClipShape;
use crate::text::skia_path;
use es_geom::Rect;
use es_model::BorderSettings;
use kurbo::{RoundedRect, RoundedRectRadii, Shape};
use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, Path, PathBuilder, Pattern, Pixmap, SpreadMode,
    Transform,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub image: ClipShape,
    pub card: ClipShape,
}

pub fn frame(layout: &Layout) -> Frame {
    Frame {
        image: ClipShape {
            rect: layout.image,
            radii: layout.image_radii,
        },
        card: ClipShape {
            rect: layout.card,
            radii: layout.card_radii,
        },
    }
}

/// Radii are limited to half the shorter edge so neighbouring corners never overlap.
pub fn rounded_path(rect: Rect, radii: [f64; 4]) -> Option<Path> {
    let cap = (rect.width().min(rect.height()) / 2.0).max(0.0);
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0.0, cap));
    let rr = RoundedRect::from_rect(rect, RoundedRectRadii::new(tl, tr, br, bl));
    skia_path(&rr.to_path(0.05))
}

pub fn paint_border(target: &mut Pixmap, frame: &Frame, border: &BorderSettings) {
    let (Some(card), Some(image)) = (
        rounded_path(frame.card.rect, frame.card.radii),
        rounded_path(frame.image.rect, frame.image.radii),
    ) else {
        return;
    };
    let mut ring = PathBuilder::new();
    ring.push_path(&card);
    ring.push_path(&image);
    let Some(ring) = ring.finish() else { return };
    let c = &border.color;
    let alpha = (c.alpha * border.opacity).clamp(0.0, 1.0) as f32;
    let mut paint = Paint::default();
    paint.set_color(
        Color::from_rgba(
            c.red.clamp(0.0, 1.0) as f32,
            c.green.clamp(0.0, 1.0) as f32,
            c.blue.clamp(0.0, 1.0) as f32,
            alpha,
        )
        .unwrap_or(Color::WHITE),
    );
    target.fill_path(
        &ring,
        &paint,
        FillRule::EvenOdd,
        Transform::identity(),
        None,
    );
}

pub fn paint_image(target: &mut Pixmap, image: &Pixmap, frame: &Frame) {
    let Some(path) = rounded_path(frame.image.rect, frame.image.radii) else {
        return;
    };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.shader = Pattern::new(
        image.as_ref(),
        SpreadMode::Pad,
        FilterQuality::Nearest,
        1.0,
        Transform::from_translate(frame.image.rect.x0 as f32, frame.image.rect.y0 as f32),
    );
    target.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}
