use es_geom::{Point, Rect, Size};
use es_model::platform::Frame;
use std::sync::Arc;

pub const HANDLE_RADIUS: f64 = 8.0;
pub const MIN_SIZE: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Move,
}

impl Handle {
    pub const EDGES: [Handle; 8] = [
        Handle::TopLeft,
        Handle::Top,
        Handle::TopRight,
        Handle::Right,
        Handle::BottomRight,
        Handle::Bottom,
        Handle::BottomLeft,
        Handle::Left,
    ];

    fn moves_left(self) -> bool {
        matches!(self, Handle::TopLeft | Handle::Left | Handle::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Handle::TopRight | Handle::Right | Handle::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Handle::TopLeft | Handle::Top | Handle::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(
            self,
            Handle::BottomLeft | Handle::Bottom | Handle::BottomRight
        )
    }

    pub fn point(self, r: Rect) -> Point {
        let x = if self.moves_left() {
            r.x0
        } else if self.moves_right() {
            r.x1
        } else {
            (r.x0 + r.x1) / 2.0
        };
        let y = if self.moves_top() {
            r.y0
        } else if self.moves_bottom() {
            r.y1
        } else {
            (r.y0 + r.y1) / 2.0
        };
        Point::new(x, y)
    }
}

/// The nearest handle within `HANDLE_RADIUS` wins over moving, so small selections stay resizable.
pub fn handle_at(p: Point, r: Rect, movable: bool) -> Option<Handle> {
    let nearest = Handle::EDGES
        .iter()
        .map(|&h| (h, h.point(r).distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match nearest {
        Some((h, d)) if d <= HANDLE_RADIUS => Some(h),
        _ if movable && r.contains(p) => Some(Handle::Move),
        _ => None,
    }
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

pub fn apply(handle: Handle, from: Point, to: Point, r: Rect, bounds: Rect) -> Rect {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if handle == Handle::Move {
        let x0 = clamp(r.x0 + dx, bounds.x0, bounds.x1 - r.width());
        let y0 = clamp(r.y0 + dy, bounds.y0, bounds.y1 - r.height());
        return Rect::new(x0, y0, x0 + r.width(), y0 + r.height());
    }
    let shift = |on: bool, d: f64| if on { d } else { 0.0 };
    let x0 = clamp(r.x0 + shift(handle.moves_left(), dx), bounds.x0, bounds.x1);
    let x1 = clamp(r.x1 + shift(handle.moves_right(), dx), bounds.x0, bounds.x1);
    let y0 = clamp(r.y0 + shift(handle.moves_top(), dy), bounds.y0, bounds.y1);
    let y1 = clamp(
        r.y1 + shift(handle.moves_bottom(), dy),
        bounds.y0,
        bounds.y1,
    );
    Rect::new(x0, y0, x1, y1).abs()
}

pub fn drawn(a: Point, b: Point, bounds: Rect) -> Rect {
    let c = |p: Point| {
        Point::new(
            clamp(p.x, bounds.x0, bounds.x1),
            clamp(p.y, bounds.y0, bounds.y1),
        )
    };
    Rect::from_points(c(a), c(b))
}

pub fn counts(r: Rect) -> bool {
    r.width() > MIN_SIZE && r.height() > MIN_SIZE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Rounds each edge, not origin and size separately: rounding the size alone loses a pixel at fractional scales.
pub fn to_pixels(local: Rect, logical: Size, px: (u32, u32)) -> Option<PixelRect> {
    if logical.width <= 0.0 || logical.height <= 0.0 {
        return None;
    }
    let (sx, sy) = (
        f64::from(px.0) / logical.width,
        f64::from(px.1) / logical.height,
    );
    let edge = |v: f64, s: f64, max: u32| clamp((v * s).round(), 0.0, f64::from(max)) as u32;
    let r = local.abs();
    let (x0, x1) = (edge(r.x0, sx, px.0), edge(r.x1, sx, px.0));
    let (y0, y1) = (edge(r.y0, sy, px.1), edge(r.y1, sy, px.1));
    (x1 > x0 && y1 > y0).then_some(PixelRect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// `local` is in the frame's own logical space, with its origin at the output's top-left.
pub fn crop(frame: &Frame, local: Rect) -> Option<Frame> {
    let p = to_pixels(local, frame.logical.size(), frame.size_px)?;
    let stride = frame.size_px.0 as usize * 4;
    if frame.bgra.len() < stride * frame.size_px.1 as usize {
        return None;
    }
    let row = p.w as usize * 4;
    let mut bgra = Vec::with_capacity(row * p.h as usize);
    for y in p.y..p.y + p.h {
        let start = y as usize * stride + p.x as usize * 4;
        bgra.extend_from_slice(&frame.bgra[start..start + row]);
    }
    let origin = Point::new(frame.logical.x0, frame.logical.y0);
    Some(Frame {
        output: frame.output.clone(),
        size_px: (p.w, p.h),
        logical: local + origin.to_vec2(),
        scale: frame.scale,
        bgra: Arc::from(bgra),
        transform: frame.transform,
        captured_at: frame.captured_at,
    })
}
