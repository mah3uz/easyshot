use crate::session::doc::DocModel;
use crate::session::viewport::ViewportModel;
use es_anno::crop::{AspectPreset, CropHandle, CropState, ImageOp};
use es_anno::{EditHistory, Snapshot};
use es_geom::{Point, Rect};
use es_render::image_ops::{self, ImageTransform};
use gpui::{App, Context, Entity, EventEmitter};
use image::RgbaImage;
use std::collections::VecDeque;
use std::sync::Arc;

/// Image steps kept across undo and redo; the oldest goes first.
pub const IMAGE_UNDO_STEPS: usize = 16;
/// Pixel bytes the image steps may hold; the newest step is kept even when it alone is larger.
pub const IMAGE_UNDO_BYTES: usize = 768 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropTarget {
    Move,
    Handle(CropHandle),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CropEvent {
    Started,
    Ended,
    /// The image was cropped; the rectangle is where the kept region sat in the canvas element.
    Applied(Rect),
    /// Reset or an aspect change moved the crop; the rectangle is where it was, in image pixels.
    Reshaped(Rect),
    /// Crop mode ended without cropping; the rectangle is where the crop was, in image pixels.
    Cancelled(Rect),
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    target: CropTarget,
    last: Point,
    grab: kurbo::Vec2,
}

/// A base image and its shapes, with the engine history that belongs to them.
struct ImageStep {
    base: Arc<RgbaImage>,
    shapes: Snapshot,
    history: EditHistory,
}

pub struct CropModel {
    doc: Entity<DocModel>,
    viewport: Entity<ViewportModel>,
    state: Option<CropState>,
    drag: Option<Drag>,
    undo: VecDeque<ImageStep>,
    redo: Vec<ImageStep>,
    redo_revision: u64,
}

impl EventEmitter<CropEvent> for CropModel {}

pub fn transform_of(op: ImageOp) -> ImageTransform {
    match op {
        ImageOp::RotateLeft => ImageTransform::RotateLeft,
        ImageOp::RotateRight => ImageTransform::RotateRight,
        ImageOp::FlipHorizontal => ImageTransform::FlipHorizontal,
        ImageOp::FlipVertical => ImageTransform::FlipVertical,
    }
}

impl CropModel {
    pub fn new(doc: Entity<DocModel>, viewport: Entity<ViewportModel>) -> Self {
        Self {
            doc,
            viewport,
            state: None,
            drag: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            redo_revision: 0,
        }
    }

    pub fn is_cropping(&self) -> bool {
        self.state.is_some()
    }

    pub fn state(&self) -> Option<&CropState> {
        self.state.as_ref()
    }

    pub fn dragging(&self) -> Option<CropTarget> {
        self.drag.map(|d| d.target)
    }

    /// Commits text editing, clears the selection and starts on the whole image.
    pub fn begin(&mut self, cx: &mut Context<Self>) {
        if self.state.is_some() {
            return;
        }
        self.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.stop_text());
            d.apply(cx, |e| e.select([]));
        });
        let size = self.doc.read(cx).image_size();
        self.state = Some(CropState::new(size));
        self.viewport.update(cx, |v, cx| v.fit(cx));
        cx.emit(CropEvent::Started);
        cx.notify();
    }

    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        if self.is_cropping() {
            self.cancel(cx);
        } else {
            self.begin(cx);
        }
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.state.take() {
            self.drag = None;
            cx.emit(CropEvent::Cancelled(state.rect()));
            cx.emit(CropEvent::Ended);
            cx.notify();
        }
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.reshape(cx, CropState::reset);
    }

    pub fn set_aspect(&mut self, aspect: AspectPreset, cx: &mut Context<Self>) {
        self.reshape(cx, |s| s.set_aspect(aspect));
    }

    fn reshape(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut CropState)) {
        if let Some(s) = self.state.as_mut() {
            let from = s.rect();
            f(s);
            if s.rect() != from {
                cx.emit(CropEvent::Reshaped(from));
            }
            cx.notify();
        }
    }

    /// Leaves crop mode; returns whether the image changed, which a whole-image rect does not.
    pub fn apply(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(state) = self.state.take() else {
            return false;
        };
        self.drag = None;
        cx.emit(CropEvent::Ended);
        cx.notify();
        let Some(result) = state.apply() else {
            return false;
        };
        let r = result.region;
        let base = self.doc.read(cx).base().clone();
        let Some(cropped) = image_ops::crop(
            &base,
            i64::from(r.x),
            i64::from(r.y),
            i64::from(r.width),
            i64::from(r.height),
        ) else {
            return false;
        };
        let vp = self.viewport.read(cx).viewport();
        let at = |x: u32, y: u32| {
            Point::new(
                vp.origin.x + f64::from(x) * vp.scale,
                vp.origin.y + f64::from(y) * vp.scale,
            )
        };
        let (a, b) = (at(r.x, r.y), at(r.x + r.width, r.y + r.height));
        self.edit(cropped, |shapes| result.map.apply_to(shapes), cx);
        cx.emit(CropEvent::Applied(Rect::new(a.x, a.y, b.x, b.y)));
        true
    }

    /// Rotates or flips the base image and carries every annotation with its pixels.
    pub fn transform(&mut self, op: ImageOp, cx: &mut Context<Self>) -> bool {
        if self.is_cropping() {
            return false;
        }
        let (base, size) = {
            let d = self.doc.read(cx);
            (d.base().clone(), d.image_size())
        };
        let turned = transform_of(op).apply(&base);
        self.edit(turned, |shapes| op.apply_to(size, shapes), cx);
        true
    }

    pub fn press(&mut self, target: CropTarget, at: Point) {
        let Some(s) = self.state.as_ref() else {
            return;
        };
        let grab = match target {
            CropTarget::Move => kurbo::Vec2::ZERO,
            CropTarget::Handle(h) => {
                let r = s.rect();
                let u = h.unit_point();
                Point::new(r.x0 + u.x * r.width(), r.y0 + u.y * r.height()) - at
            }
        };
        self.drag = Some(Drag {
            target,
            last: at,
            grab,
        });
    }

    /// `from_center` resizes symmetrically about the centre.
    pub fn drag_to(&mut self, at: Point, from_center: bool, cx: &mut Context<Self>) {
        let (Some(drag), Some(s)) = (self.drag.as_mut(), self.state.as_mut()) else {
            return;
        };
        match drag.target {
            CropTarget::Move => s.move_by(at - drag.last),
            CropTarget::Handle(h) => s.resize(h, at + drag.grab, from_center),
        }
        drag.last = at;
        cx.notify();
    }

    pub fn release(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self, cx: &App) -> bool {
        !self.redo.is_empty() && self.redo_revision == self.doc.read(cx).editor().history_revision()
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        if self.is_cropping() {
            return false;
        }
        let Some(step) = self.undo.pop_back() else {
            return false;
        };
        let back = self.restore(step, cx);
        self.redo.push(back);
        self.redo_revision = self.doc.read(cx).editor().history_revision();
        true
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        if self.is_cropping() || !self.can_redo(cx) {
            return false;
        }
        let Some(step) = self.redo.pop() else {
            return false;
        };
        let back = self.restore(step, cx);
        self.undo.push_back(back);
        true
    }

    fn edit(
        &mut self,
        base: RgbaImage,
        remap: impl FnOnce(&mut [es_model::AnnoShape]),
        cx: &mut Context<Self>,
    ) {
        self.doc.update(cx, |d, cx| d.apply(cx, |e| e.stop_text()));
        let mut shapes = self.doc.read(cx).editor().doc().snapshot();
        remap(&mut shapes.shapes);
        let mut step = self.restore(
            ImageStep {
                base: Arc::new(base),
                shapes,
                history: EditHistory::default(),
            },
            cx,
        );
        step.history.forget_redo();
        self.undo.push_back(step);
        self.redo.clear();
        self.trim();
    }

    /// Installs `step` and returns what it replaced.
    fn restore(&mut self, step: ImageStep, cx: &mut Context<Self>) -> ImageStep {
        let back = self.doc.update(cx, |d, cx| {
            let mut out = (Snapshot::default(), EditHistory::default());
            d.apply(cx, |e| {
                let (ch, shapes, history) = e.swap_document(step.shapes, step.history);
                out = (shapes, history);
                ch
            });
            let base = d.base().clone();
            d.replace_base(step.base, cx);
            ImageStep {
                base,
                shapes: out.0,
                history: out.1,
            }
        });
        self.viewport.update(cx, |v, cx| v.image_changed(cx));
        cx.notify();
        back
    }

    fn trim(&mut self) {
        let bytes = |s: &ImageStep| s.base.as_raw().len();
        let mut held: usize = self.undo.iter().chain(&self.redo).map(bytes).sum();
        while self.undo.len() > 1
            && (self.undo.len() + self.redo.len() > IMAGE_UNDO_STEPS || held > IMAGE_UNDO_BYTES)
        {
            if let Some(old) = self.undo.pop_front() {
                held -= bytes(&old);
            }
        }
    }

    /// Pixel bytes held by the image undo and redo steps.
    pub fn held_bytes(&self) -> usize {
        self.undo
            .iter()
            .chain(&self.redo)
            .map(|s| s.base.as_raw().len())
            .sum()
    }
}
