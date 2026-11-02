mod chrome;
mod element;
pub mod grid;
mod input;
mod live;
pub mod paths;
mod prepare;
pub mod redaction;
pub mod tiles;
pub mod underlay;

pub use element::CanvasElement;

use crate::session::EditorSession;
use crate::session::doc::DocModel;
use crate::session::viewport::{ViewportModel, WHEEL_ZOOM_RATE};
use es_anno::{DrawItem, Viewport};
use es_geom::Point;
use es_model::{AnnoShapeId, BackgroundSettings};
use es_ui::cursors::Cursors;
use gpui::{
    App, Bounds, Context, CursorStyle, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, MouseButton, MouseDownEvent, ParentElement, PinchEvent, Pixels,
    Render, RenderImage, ScrollEvent, Styled, Subscription, Task, Window, div, px,
};
use grid::GridKey;
use live::LiveCache;
use paths::{Bucket, Painted, Placement};
use redaction::Redactions;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tiles::BaseTiles;
use underlay::UnderlayKey;

const WHEEL_LINE: Pixels = px(20.);

/// Paints blur and pixelate patches; receives the page-space redaction items in z-order.
pub type RedactionPainter =
    Box<dyn Fn(&[DrawItem], &Viewport, Bounds<Pixels>, &mut Window, &mut App)>;

/// What the last frame did, for tests and benches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub tiles_painted: usize,
    pub shapes_tessellated: usize,
    pub shapes_placed: usize,
    pub paths_painted: usize,
    pub live_chunks_tessellated: usize,
}

struct ShapeEntry {
    revision: u64,
    bucket: Bucket,
    built: Vec<Painted>,
    placement: Option<Placement>,
    placed: Vec<Painted>,
}

struct SpotEntry {
    holes: Vec<(AnnoShapeId, u64)>,
    bucket: Bucket,
    built: Vec<Painted>,
    placement: Option<Placement>,
    placed: Vec<Painted>,
}

pub struct EditorCanvas {
    doc: Entity<DocModel>,
    viewport: Entity<ViewportModel>,
    background: BackgroundSettings,
    focus: FocusHandle,
    shapes: HashMap<AnnoShapeId, ShapeEntry>,
    spotlight: Option<SpotEntry>,
    grid: Option<(GridKey, Arc<RenderImage>)>,
    tiles: Option<BaseTiles>,
    underlay: Option<(UnderlayKey, Arc<RenderImage>)>,
    underlay_pending: Option<UnderlayKey>,
    underlay_task: Option<Task<()>>,
    retired: Vec<Arc<RenderImage>>,
    redaction_painter: Option<RedactionPainter>,
    redactions: Rc<RefCell<Redactions>>,
    live: LiveCache,
    cursors: Cursors,
    cursor: CursorStyle,
    bounds: Bounds<Pixels>,
    space_held: bool,
    pan_from: Option<gpui::Point<Pixels>>,
    stats: FrameStats,
    _subscriptions: Vec<Subscription>,
}

impl EditorCanvas {
    pub fn new(session: &EditorSession, cx: &mut Context<Self>) -> Self {
        let background = session.doc.read(cx).background().clone();
        let subscriptions = vec![
            cx.observe(&session.doc, |_, _, cx| cx.notify()),
            cx.observe(&session.viewport, |_, _, cx| cx.notify()),
        ];
        let redactions = Rc::new(RefCell::new(Redactions::default()));
        let painter = redaction::painter(redactions.clone(), session.doc.clone(), cx.weak_entity());
        Self {
            doc: session.doc.clone(),
            viewport: session.viewport.clone(),
            background,
            focus: cx.focus_handle(),
            shapes: HashMap::new(),
            spotlight: None,
            grid: None,
            tiles: None,
            underlay: None,
            underlay_pending: None,
            underlay_task: None,
            retired: Vec::new(),
            redaction_painter: Some(painter),
            redactions,
            live: LiveCache::default(),
            cursors: crate::cursor::cursors(cx),
            cursor: CursorStyle::Arrow,
            bounds: Bounds::default(),
            space_held: false,
            pan_from: None,
            stats: FrameStats::default(),
            _subscriptions: subscriptions,
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    pub fn last_frame(&self) -> FrameStats {
        self.stats
    }

    /// The element's bounds in the window as of the last frame.
    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    /// True while the middle button or Space-held drag is panning; tools ignore the pointer then.
    pub fn is_panning(&self) -> bool {
        self.pan_from.is_some() || self.space_held
    }

    /// Whether an underlay image for the current look is on screen.
    pub fn underlay_ready(&self) -> bool {
        self.underlay_pending.is_none()
    }

    pub fn set_redaction_painter(&mut self, painter: RedactionPainter) {
        self.redaction_painter = Some(painter);
    }

    pub fn set_background(&mut self, settings: BackgroundSettings, cx: &mut Context<Self>) {
        if settings == self.background {
            return;
        }
        self.viewport
            .update(cx, |v, cx| v.set_background(&settings, cx));
        self.background = settings;
        cx.notify();
    }

    fn local(&self, p: gpui::Point<Pixels>) -> Point {
        let o = self.bounds.origin;
        Point::new(f64::from(p.x - o.x), f64::from(p.y - o.y))
    }

    fn on_scroll(&mut self, e: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let d = e.delta.pixel_delta(WHEEL_LINE);
        let (dx, dy) = (f64::from(d.x), f64::from(d.y));
        let at = self.local(e.position);
        if e.modifiers.control || e.modifiers.alt {
            let factor = (1.0 + dy * WHEEL_ZOOM_RATE).clamp(0.5, 2.0);
            self.viewport.update(cx, |v, cx| v.zoom_at(factor, at, cx));
        } else {
            self.viewport.update(cx, |v, cx| v.pan_by(dx, dy, cx));
        }
        cx.stop_propagation();
    }

    fn on_pinch(&mut self, e: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let factor = (1.0 + f64::from(e.delta)).clamp(0.5, 2.0);
        let at = self.local(e.position);
        self.viewport.update(cx, |v, cx| v.zoom_at(factor, at, cx));
        cx.stop_propagation();
    }

    fn start_pan(&mut self, e: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if e.button == MouseButton::Middle || self.space_held {
            self.pan_from = Some(e.position);
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(crate) fn drag_pan(&mut self, to: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(from) = self.pan_from.replace(to) {
            let d = to - from;
            let (dx, dy) = (f64::from(d.x), f64::from(d.y));
            self.viewport.update(cx, |v, cx| v.pan_by(dx, dy, cx));
        }
    }

    pub(crate) fn end_pan(&mut self, cx: &mut Context<Self>) {
        if self.pan_from.take().is_some() {
            cx.notify();
        }
    }

    fn on_key_down(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let editing = self.doc.read(cx).editor().editing_text_id().is_some();
        if e.keystroke.key == "space" && !editing {
            if !self.space_held {
                self.space_held = true;
                cx.notify();
            }
            cx.stop_propagation();
        }
    }

    fn on_key_up(&mut self, e: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if e.keystroke.key == "space" && self.space_held {
            self.space_held = false;
            self.pan_from = None;
            cx.notify();
        }
    }
}

impl Render for EditorCanvas {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(es_ui::theme::tokens(cx).workspace)
            .child(
                div()
                    .id("editor-canvas")
                    .track_focus(&self.focus)
                    .size_full()
                    .on_scroll(cx.listener(Self::on_scroll))
                    .on_pinch(cx.listener(Self::on_pinch))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::start_pan))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::start_pan))
                    .on_key_down(cx.listener(Self::on_key_down))
                    .on_key_up(cx.listener(Self::on_key_up))
                    .child(CanvasElement::new(cx.entity())),
            )
    }
}
