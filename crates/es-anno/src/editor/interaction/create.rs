use super::Interaction;
use crate::document::page_transform;
use crate::editor::Editor;
use crate::{Changes, PointerInfo};
use es_geom::PVec;
use es_geom::freehand::{LiveStroke, StrokeOptions};
use es_model::{
    AnnoShape, AnnoShapeId, ArrowProps, Arrowhead, DrawProps, FillStyle, GeoKind, GeoProps,
    HighlightProps, NumberedProps, RedactionKind, RedactionProps, ShapeKind, Tool,
};
use std::f64::consts::PI;

const CLICK_EXTENT: f64 = 2.0;
const DEFAULT_BOX_MIN: f64 = 48.0;
const DEFAULT_BOX_FRACTION: f64 = 0.08;
const MIN_ARROW_LENGTH: f64 = 4.0;
const NUMBERED_MIN: f64 = 24.0;
const NUMBERED_FRACTION: f64 = 0.039;
const ANGLE_STEP: f64 = PI / 12.0;
const MOUSE_PRESSURE: f64 = 0.5;

fn pressure(pointer: &PointerInfo) -> f64 {
    if pointer.is_pen {
        pointer.pressure
    } else {
        MOUSE_PRESSURE
    }
}

fn box_size(shape: &AnnoShape) -> Option<(f64, f64)> {
    match &shape.kind {
        ShapeKind::Geo(p) => Some((p.w, p.h)),
        ShapeKind::Redaction(p) => Some((p.w, p.h)),
        ShapeKind::Highlight(p) => Some((p.w, p.h)),
        _ => None,
    }
}

fn snap_angle(anchor: PVec, point: PVec) -> PVec {
    let delta = PVec::sub(point, anchor);
    let len = delta.len();
    if len <= 0.0 {
        return point;
    }
    let angle = (delta.y.atan2(delta.x) / ANGLE_STEP).round() * ANGLE_STEP;
    PVec::add(anchor, PVec::from_angle(angle, len))
}

impl Editor {
    /// Deleting the highest number frees it again; numbers in the middle leave a gap.
    pub fn next_numbered_value(&self) -> u32 {
        self.doc
            .shapes()
            .iter()
            .filter_map(|s| match &s.kind {
                ShapeKind::Numbered(p) => Some(p.value),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    fn image_edge(&self) -> f64 {
        let size = self.viewport.image_size;
        size.width.max(size.height)
    }

    fn page_stroke(&self) -> f64 {
        self.viewport.page_stroke_width(self.current.stroke_width)
    }

    fn add_shape(&mut self, shape: AnnoShape, ch: &mut Changes) -> AnnoShapeId {
        let id = shape.id;
        self.begin_fresh_gesture(ch);
        self.edit(ch, |doc| doc.add(shape));
        id
    }

    pub(super) fn begin_box(&mut self, p: PVec, ch: &mut Changes) {
        let kind = match self.tool {
            Tool::Highlight => ShapeKind::Highlight(HighlightProps { w: 1.0, h: 1.0 }),
            Tool::Blur | Tool::Pixelate => ShapeKind::Redaction(RedactionProps {
                kind: if self.tool == Tool::Blur {
                    RedactionKind::Blur
                } else {
                    RedactionKind::Pixelate
                },
                w: 1.0,
                h: 1.0,
                density: self.current.redaction_density,
            }),
            _ => ShapeKind::Geo(GeoProps {
                geo: if self.tool == Tool::Ellipse {
                    GeoKind::Ellipse
                } else {
                    GeoKind::Rectangle
                },
                fill: if self.tool == Tool::FilledRectangle {
                    FillStyle::Solid
                } else {
                    FillStyle::None
                },
                swatch: self.current.swatch.clone(),
                stroke_width: self.page_stroke(),
                w: 1.0,
                h: 1.0,
                ..GeoProps::default()
            }),
        };
        let id = self.add_shape(AnnoShape::new(p.x, p.y, kind), ch);
        self.interaction = Interaction::CreatingBox { id, origin: p };
    }

    /// Shift makes every box tool square; Alt grows the box from the press point as its centre.
    pub(super) fn resize_new_box(
        &mut self,
        id: AnnoShapeId,
        origin: PVec,
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let (dx, dy) = (p.x - origin.x, p.y - origin.y);
        let reach = if pointer.alt { 2.0 } else { 1.0 };
        let (mut w, mut h) = (dx.abs() * reach, dy.abs() * reach);
        if pointer.shift {
            w = w.max(h);
            h = w;
        }
        let place = |o: f64, d: f64, size: f64| {
            if pointer.alt {
                o - size / 2.0
            } else if d < 0.0 {
                o - size
            } else {
                o
            }
        };
        let (x, y) = (place(origin.x, dx, w), place(origin.y, dy, h));
        self.set_box(id, x, y, w, h, ch);
    }

    fn set_box(&mut self, id: AnnoShapeId, x: f64, y: f64, w: f64, h: f64, ch: &mut Changes) {
        let (w, h) = (w.max(1.0), h.max(1.0));
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                s.x = x;
                s.y = y;
                match &mut s.kind {
                    ShapeKind::Geo(p) => (p.w, p.h) = (w, h),
                    ShapeKind::Redaction(p) => (p.w, p.h) = (w, h),
                    ShapeKind::Highlight(p) => (p.w, p.h) = (w, h),
                    _ => {}
                }
            })
        });
    }

    /// A click without a drag makes a usefully sized square at the press point.
    pub(super) fn finish_box(
        &mut self,
        id: AnnoShapeId,
        origin: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let Some((w, h)) = self.doc.shape(id).and_then(box_size) else {
            return;
        };
        if w <= CLICK_EXTENT && h <= CLICK_EXTENT {
            let side = DEFAULT_BOX_MIN.max(self.image_edge() * DEFAULT_BOX_FRACTION);
            let (x, y) = if pointer.alt {
                (origin.x - side / 2.0, origin.y - side / 2.0)
            } else {
                (origin.x, origin.y)
            };
            self.set_box(id, x, y, side, side, ch);
        }
        self.set_selection([id], ch);
    }

    pub(super) fn begin_arrow(&mut self, p: PVec, ch: &mut Changes) {
        let (head_start, head_end) = if self.tool == Tool::Line {
            (Arrowhead::None, Arrowhead::None)
        } else {
            (self.current.arrowhead_start, self.current.arrowhead_end)
        };
        let props = ArrowProps {
            start: PVec::new(0.0, 0.0),
            end: PVec::new(0.0, 0.0),
            arrowhead_start: head_start,
            arrowhead_end: head_end,
            swatch: self.current.swatch.clone(),
            stroke_width: self.page_stroke(),
            ..ArrowProps::default()
        };
        let id = self.add_shape(AnnoShape::new(p.x, p.y, ShapeKind::Arrow(props)), ch);
        self.interaction = Interaction::CreatingArrow { id };
    }

    /// Shift snaps the line to the nearest 15 degrees around its start.
    pub(super) fn move_new_arrow_end(
        &mut self,
        id: AnnoShapeId,
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let Some(shape) = self.doc.shape(id) else {
            return;
        };
        let ShapeKind::Arrow(props) = &shape.kind else {
            return;
        };
        let local = page_transform(shape).inverse().apply_to_point(p);
        let end = if pointer.shift {
            snap_angle(props.start, local)
        } else {
            local
        };
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Arrow(a) = &mut s.kind {
                    a.end = PVec::new(end.x, end.y);
                }
            })
        });
    }

    /// An arrow too short to see is dropped, which also takes back its undo entry.
    pub(super) fn finish_arrow(&mut self, id: AnnoShapeId, ch: &mut Changes) {
        let too_short = match self.doc.shape(id).map(|s| &s.kind) {
            Some(ShapeKind::Arrow(a)) => PVec::dist_min(a.start, a.end, MIN_ARROW_LENGTH),
            _ => return,
        };
        if too_short {
            self.edit(ch, |doc| doc.delete(&[id]));
        } else {
            self.set_selection([id], ch);
        }
    }

    pub(super) fn begin_drawing(&mut self, p: PVec, pointer: &PointerInfo, ch: &mut Changes) {
        let first = PVec::with_z(0.0, 0.0, pressure(pointer));
        let stroke_width = self.page_stroke();
        let mut stroke = LiveStroke::new(StrokeOptions::for_draw_shape(
            pointer.is_pen,
            stroke_width,
            false,
            false,
        ));
        stroke.push(first);
        let props = DrawProps {
            points: vec![first],
            is_pen: pointer.is_pen,
            swatch: self.current.swatch.clone(),
            stroke_width,
            ..DrawProps::default()
        };
        let id = self.add_shape(AnnoShape::new(p.x, p.y, ShapeKind::Draw(props)), ch);
        self.interaction = Interaction::Drawing {
            id,
            origin: p,
            stroke: Box::new(stroke),
        };
    }

    pub(super) fn extend_drawing(
        &mut self,
        id: AnnoShapeId,
        origin: PVec,
        stroke: &mut LiveStroke,
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let local = PVec::with_z(p.x - origin.x, p.y - origin.y, pressure(pointer));
        if stroke.push(local).is_none() {
            return;
        }
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Draw(d) = &mut s.kind {
                    d.points.push(local);
                }
            })
        });
    }

    /// A stroke ending near its start closes and fills; a single point draws nothing.
    pub(super) fn finish_drawing(&mut self, id: AnnoShapeId, ch: &mut Changes) {
        let Some(ShapeKind::Draw(d)) = self.doc.shape(id).map(|s| &s.kind) else {
            return;
        };
        if d.points.len() < 2 {
            self.edit(ch, |doc| doc.delete(&[id]));
            return;
        }
        let closed = match (d.points.first(), d.points.last()) {
            (Some(a), Some(b)) => {
                d.points.len() > 3 && PVec::dist_min(*a, *b, d.stroke_width * 2.0)
            }
            _ => false,
        };
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Draw(d) = &mut s.kind {
                    d.is_complete = true;
                    d.is_closed = closed;
                }
            })
        });
    }

    pub(super) fn create_numbered(&mut self, p: PVec, ch: &mut Changes) {
        let diameter = NUMBERED_MIN.max(self.image_edge() * NUMBERED_FRACTION);
        let props = NumberedProps {
            value: self.next_numbered_value(),
            diameter,
            swatch: self.current.swatch.clone(),
        };
        let shape = AnnoShape::new(
            p.x - diameter / 2.0,
            p.y - diameter / 2.0,
            ShapeKind::Numbered(props),
        );
        let id = self.add_shape(shape, ch);
        self.set_selection([id], ch);
        self.end_gesture(ch);
    }
}
