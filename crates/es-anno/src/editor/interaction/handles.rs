use super::clamp::all_within;
use super::{Handle, Interaction};
use crate::document::Document;
use crate::document::page_transform;
use crate::editor::{Editor, SelectionBounds};
use crate::{Changes, PointerInfo, ShapeId};
use es_geom::{Mat, PVec, Point, Size};
use es_model::{AnnoShape, DrawProps, HighlighterProps, ShapeKind, TextProps, Tool};
use std::f64::consts::PI;

const HIT_RADIUS: f64 = 9.0;
const ROTATE_OFFSET: f64 = 14.0;
const ROTATE_STEP: f64 = PI / 12.0;
const MIN_SCALE: f64 = 0.01;
const MIN_BOX: f64 = 1.0;
const MIN_DIAMETER: f64 = 8.0;
const MIN_FONT_SIZE: f64 = 4.0;
const MIN_TEXT_WIDTH: f64 = 16.0;

const RESIZE_HANDLES: [Handle; 8] = [
    Handle::TopLeft,
    Handle::Top,
    Handle::TopRight,
    Handle::Right,
    Handle::BottomRight,
    Handle::Bottom,
    Handle::BottomLeft,
    Handle::Left,
];

const CORNERS: [PVec; 4] = [
    PVec::new(0.0, 0.0),
    PVec::new(1.0, 0.0),
    PVec::new(1.0, 1.0),
    PVec::new(0.0, 1.0),
];

impl Handle {
    pub(super) fn position(self) -> Option<PVec> {
        Some(match self {
            Handle::TopLeft => PVec::new(0.0, 0.0),
            Handle::Top => PVec::new(0.5, 0.0),
            Handle::TopRight => PVec::new(1.0, 0.0),
            Handle::Right => PVec::new(1.0, 0.5),
            Handle::BottomRight => PVec::new(1.0, 1.0),
            Handle::Bottom => PVec::new(0.5, 1.0),
            Handle::BottomLeft => PVec::new(0.0, 1.0),
            Handle::Left => PVec::new(0.0, 0.5),
            Handle::Rotate { corner } => *CORNERS.get(usize::from(corner))?,
            Handle::ArrowStart | Handle::ArrowMiddle | Handle::ArrowEnd => return None,
        })
    }

    fn is_corner(self) -> bool {
        matches!(
            self,
            Handle::TopLeft | Handle::TopRight | Handle::BottomRight | Handle::BottomLeft
        )
    }
}

fn scale_on(scales: bool, pointer: f64, anchor: f64, grip: f64) -> f64 {
    if scales && grip != anchor {
        (pointer - anchor) / (grip - anchor)
    } else {
        1.0
    }
}

/// Flipping through zero is allowed; collapsing to zero is not.
fn floor_scale(s: f64) -> f64 {
    if s.abs() >= MIN_SCALE {
        s
    } else if s < 0.0 {
        -MIN_SCALE
    } else {
        MIN_SCALE
    }
}

/// The gesture is interpolated from its start, so the largest fitting fraction is where it stops.
fn largest_fit(mut fits: impl FnMut(f64) -> bool) -> f64 {
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn linear(m: Mat, v: PVec) -> PVec {
    PVec::sub(m.apply_to_point(v), m.apply_to_point(PVec::ZERO))
}

struct Scale {
    x: f64,
    y: f64,
    anchor: PVec,
    selection: Mat,
    width_only: bool,
}

/// A negative scale puts the box on the other side of its scaled origin, along the shape's axes.
fn scaled(shape: &AnnoShape, k: &Scale, measure: impl Fn(&TextProps) -> Size) -> AnnoShape {
    let own = page_transform(shape);
    let to_selection = Mat::multiply(k.selection.inverse(), own);
    let o = to_selection.apply_to_point(PVec::ZERO);
    let origin = PVec::new(
        k.anchor.x + (o.x - k.anchor.x) * k.x,
        k.anchor.y + (o.y - k.anchor.y) * k.y,
    );
    let r = to_selection.rotation();
    let (c, s) = (r.cos().abs(), r.sin().abs());
    let (sx, sy) = (k.x * c + k.y * s, k.y * c + k.x * s);
    let mean = (sx.abs() + sy.abs()) / 2.0;
    let mut next = shape.clone();
    let extent = match &mut next.kind {
        ShapeKind::Geo(p) => {
            (p.w, p.h) = ((p.w * sx).abs().max(MIN_BOX), (p.h * sy).abs().max(MIN_BOX));
            Some((p.w, p.h))
        }
        ShapeKind::Redaction(p) => {
            (p.w, p.h) = ((p.w * sx).abs().max(MIN_BOX), (p.h * sy).abs().max(MIN_BOX));
            Some((p.w, p.h))
        }
        ShapeKind::Highlight(p) => {
            (p.w, p.h) = ((p.w * sx).abs().max(MIN_BOX), (p.h * sy).abs().max(MIN_BOX));
            Some((p.w, p.h))
        }
        ShapeKind::Numbered(p) => {
            p.diameter = (p.diameter * mean).abs().max(MIN_DIAMETER);
            Some((p.diameter, p.diameter))
        }
        ShapeKind::Draw(DrawProps { points, .. })
        | ShapeKind::Highlighter(HighlighterProps { points, .. }) => {
            for q in points {
                (q.x, q.y) = (q.x * sx, q.y * sy);
            }
            None
        }
        ShapeKind::Arrow(p) => {
            p.start = PVec::new(p.start.x * sx, p.start.y * sy);
            p.end = PVec::new(p.end.x * sx, p.end.y * sy);
            p.bend *= mean;
            if sx * sy < 0.0 {
                p.bend = -p.bend;
            }
            None
        }
        ShapeKind::Text(p) => {
            if k.width_only {
                let width = if p.auto_size { measure(p).width } else { p.w };
                p.w = (width * sx).abs().max(MIN_TEXT_WIDTH);
                p.auto_size = false;
            } else {
                p.font_size = (p.font_size * mean).max(MIN_FONT_SIZE);
                if !p.auto_size {
                    p.w = (p.w * mean).max(MIN_TEXT_WIDTH);
                }
            }
            let size = measure(p);
            Some((size.width, size.height))
        }
    };
    let flip = extent.map_or(PVec::ZERO, |(w, h)| {
        PVec::new(
            if sx < 0.0 { -w } else { 0.0 },
            if sy < 0.0 { -h } else { 0.0 },
        )
    });
    let page = PVec::add(k.selection.apply_to_point(origin), linear(own, flip));
    (next.x, next.y) = (page.x, page.y);
    next
}

impl Editor {
    pub(super) fn screen_of(&self, page: PVec) -> PVec {
        let s = self.viewport.page_to_screen(Point::new(page.x, page.y));
        PVec::new(s.x, s.y)
    }

    pub(super) fn rotate_point(&self, bounds: &SelectionBounds, corner: PVec) -> (PVec, PVec) {
        let at = self.screen_of(bounds.page_point(corner));
        let center = self.screen_of(bounds.page_center());
        let outward = PVec::sub(at, center).uni();
        (PVec::add(at, PVec::mul(outward, ROTATE_OFFSET)), center)
    }

    /// Tested in screen space so handles are as easy to grab at every zoom; resize before rotate.
    pub fn handle_at(&self, screen: Point) -> Option<Handle> {
        if self.tool != Tool::Select {
            return None;
        }
        let screen = PVec::new(screen.x, screen.y);
        let near = |p: PVec| PVec::dist(p, screen) <= HIT_RADIUS;
        if let Some(shape) = self.lone_arrow() {
            let info = self.doc.arrow_info(shape.id)?;
            let t = page_transform(shape);
            let [start, middle, end] = info.handles();
            return [
                (Handle::ArrowStart, start),
                (Handle::ArrowMiddle, middle),
                (Handle::ArrowEnd, end),
            ]
            .into_iter()
            .find(|(_, p)| near(self.screen_of(t.apply_to_point(*p))))
            .map(|(h, _)| h);
        }
        let bounds = self.selection_bounds()?;
        let resize = RESIZE_HANDLES.into_iter().find(|h| {
            h.position()
                .is_some_and(|n| near(self.screen_of(bounds.page_point(n))))
        });
        resize.or_else(|| {
            (0..4u8).map(|corner| Handle::Rotate { corner }).find(|h| {
                h.position()
                    .is_some_and(|n| near(self.rotate_point(&bounds, n).0))
            })
        })
    }

    fn selected_shapes(&self) -> Vec<AnnoShape> {
        self.selected_in_order()
            .into_iter()
            .filter_map(|id| self.doc.shape(id).cloned())
            .collect()
    }

    pub(super) fn begin_handle(&mut self, handle: Handle, p: PVec, ch: &mut Changes) {
        let cursor = self.handle_cursor(handle);
        let interaction = match handle {
            Handle::ArrowStart | Handle::ArrowMiddle | Handle::ArrowEnd => {
                let Some(id) = self.lone_arrow().map(|s| s.id) else {
                    return;
                };
                Interaction::DraggingArrowHandle { id, handle, cursor }
            }
            Handle::Rotate { .. } => {
                let Some(bounds) = self.selection_bounds() else {
                    return;
                };
                let center = bounds.page_center();
                Interaction::Rotating {
                    center,
                    start_angle: center.angle(p),
                    initial: self.selected_shapes(),
                }
            }
            _ => {
                let Some(bounds) = self.selection_bounds() else {
                    return;
                };
                Interaction::Resizing {
                    handle,
                    bounds,
                    initial: self.selected_shapes(),
                    cursor,
                }
            }
        };
        self.begin_fresh_gesture(ch);
        self.interaction = interaction;
    }

    /// Every move starts again from the press, so scaling never accumulates error.
    pub(super) fn resize(
        &mut self,
        handle: Handle,
        bounds: &SelectionBounds,
        initial: &[AnnoShape],
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let Some(at) = handle.position() else {
            return;
        };
        let b = bounds.bbox;
        let fixed = if pointer.alt {
            PVec::new(0.5, 0.5)
        } else {
            PVec::new(1.0 - at.x, 1.0 - at.y)
        };
        let anchor = PVec::new(b.x + b.w * fixed.x, b.y + b.h * fixed.y);
        let grip = PVec::new(b.x + b.w * at.x, b.y + b.h * at.y);
        let local = bounds.transform.inverse().apply_to_point(p);
        let mut sx = scale_on(at.x != 0.5, local.x, anchor.x, grip.x);
        let mut sy = scale_on(at.y != 0.5, local.y, anchor.y, grip.y);
        if pointer.shift && handle.is_corner() {
            let m = sx.abs().max(sy.abs());
            sx = if sx < 0.0 { -m } else { m };
            sy = if sy < 0.0 { -m } else { m };
        }
        let image = self.image_rect();
        let tied = pointer.shift && handle.is_corner();
        let apply = |doc: &mut Document, tx: f64, ty: f64| -> Vec<ShapeId> {
            let k = Scale {
                x: floor_scale(1.0 + (sx - 1.0) * tx),
                y: floor_scale(1.0 + (sy - 1.0) * ty),
                anchor,
                selection: bounds.transform,
                width_only: at.y == 0.5,
            };
            let mut dirty = Vec::new();
            for shape in initial {
                let next = scaled(shape, &k, |t| doc.measure_text(t));
                dirty.extend(doc.update(shape.id, |s| *s = next));
            }
            dirty
        };
        self.edit(ch, |doc| {
            let dirty = apply(doc, 1.0, 1.0);
            let Some(image) = image else {
                return dirty;
            };
            let fits = |doc: &Document| all_within(doc, image, initial.iter().map(|s| s.id));
            if fits(doc) {
                return dirty;
            }
            if !tied {
                let tx = largest_fit(|t| {
                    apply(doc, t, 0.0);
                    fits(doc)
                });
                let ty = largest_fit(|t| {
                    apply(doc, 0.0, t);
                    fits(doc)
                });
                let dirty = apply(doc, tx, ty);
                if fits(doc) {
                    return dirty;
                }
            }
            let t = largest_fit(|t| {
                apply(doc, t, t);
                fits(doc)
            });
            apply(doc, t, t)
        });
    }

    /// Shift snaps the turn, not the resulting angle, to 15 degree steps.
    pub(super) fn rotate(
        &mut self,
        center: PVec,
        start_angle: f64,
        initial: &[AnnoShape],
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let mut delta = center.angle(p) - start_angle;
        if pointer.shift {
            delta = (delta / ROTATE_STEP).round() * ROTATE_STEP;
        }
        let image = self.image_rect();
        let apply = |doc: &mut Document, t: f64| -> Vec<ShapeId> {
            let mut dirty = Vec::new();
            for shape in initial {
                let at = PVec::rot_with(PVec::new(shape.x, shape.y), center, delta * t);
                dirty.extend(doc.update(shape.id, |s| {
                    (s.x, s.y) = (at.x, at.y);
                    s.rotation = shape.rotation + delta * t;
                }));
            }
            dirty
        };
        self.edit(ch, |doc| {
            let dirty = apply(doc, 1.0);
            let Some(image) = image else {
                return dirty;
            };
            let fits = |doc: &Document| all_within(doc, image, initial.iter().map(|s| s.id));
            if fits(doc) {
                return dirty;
            }
            let t = largest_fit(|t| {
                apply(doc, t);
                fits(doc)
            });
            apply(doc, t)
        });
    }
}
