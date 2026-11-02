use es_anno::Viewport;
use es_anno::document::page_transform;
use es_geom::{Affine, Point, Rect};
use es_model::AnnoShape;
use es_text::TextLayout;
use gpui::{Bounds, Pixels, point, px, size};
use std::ops::Range;

/// The shape's local text frame to window pixels, rotation and mirroring included.
pub fn to_window(shape: &AnnoShape, vp: &Viewport, origin: gpui::Point<Pixels>) -> Affine {
    let m = page_transform(shape);
    Affine::translate((
        f64::from(origin.x) + vp.origin.x,
        f64::from(origin.y) + vp.origin.y,
    )) * Affine::scale(vp.scale)
        * Affine::new([m.a, m.b, m.c, m.d, m.e, m.f])
}

/// An empty text has no lines, so its caret sits where the alignment anchors the first one.
fn empty_caret(layout: &TextLayout, width: f64) -> Rect {
    let x = match layout.style.align {
        es_text::TextAlign::Start => 0.0,
        es_text::TextAlign::Middle => layout.size.width / 2.0,
        es_text::TextAlign::End => layout.size.width,
    };
    Rect::new(x, 0.0, x + width, layout.size.height)
}

pub fn caret(layout: &TextLayout, index: usize, width: f64) -> Rect {
    if layout.lines.is_empty() {
        return empty_caret(layout, width);
    }
    let r = layout.caret_rect(index, width);
    Rect::new(r.x0 - width / 2.0, r.y0, r.x1 - width / 2.0, r.y1)
}

/// `index` measured along one line, even where it ends that line and starts the next.
fn x_in_line(layout: &TextLayout, line: usize, index: usize) -> f64 {
    let lb = &layout.lines[line];
    let mut x = lb.x;
    for g in layout.glyphs.iter().filter(|g| g.line == line) {
        if index <= g.range.start {
            return g.x;
        }
        if index < g.range.end {
            let span = (g.range.end - g.range.start) as f64;
            return g.x + g.advance * (index - g.range.start) as f64 / span;
        }
        x = g.x + g.advance;
    }
    x
}

/// One rect per line the range covers, in the local frame.
pub fn spans(layout: &TextLayout, range: &Range<usize>) -> Vec<Rect> {
    if range.is_empty() {
        return Vec::new();
    }
    let last = layout.lines.len().saturating_sub(1);
    layout
        .lines
        .iter()
        .enumerate()
        .filter_map(|(i, lb)| {
            let end = if i == last { usize::MAX } else { lb.range.end };
            if range.end <= lb.range.start || range.start >= end {
                return None;
            }
            let x0 = x_in_line(layout, i, range.start.max(lb.range.start));
            let x1 = if range.end > lb.range.end && i < last {
                x_in_line(layout, i, lb.range.end).max(lb.x + lb.width)
            } else {
                x_in_line(layout, i, range.end.min(lb.range.end))
            };
            (x1 > x0).then(|| Rect::new(x0, lb.top, x1, lb.top + lb.height))
        })
        .collect()
}

pub fn line_of(layout: &TextLayout, index: usize) -> Option<usize> {
    if layout.lines.is_empty() {
        return None;
    }
    Some(layout.caret_x(index).0)
}

/// The window box around a local rect after the shape's rotation.
pub fn window_bounds(r: Rect, ts: Affine) -> Bounds<Pixels> {
    let corners = [
        Point::new(r.x0, r.y0),
        Point::new(r.x1, r.y0),
        Point::new(r.x1, r.y1),
        Point::new(r.x0, r.y1),
    ]
    .map(|p| ts * p);
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in corners {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Bounds::new(
        point(px(x0 as f32), px(y0 as f32)),
        size(px((x1 - x0) as f32), px((y1 - y0) as f32)),
    )
}
