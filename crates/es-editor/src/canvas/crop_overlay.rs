use super::EditorCanvas;
use super::paths::{self, Bucket, Painted};
use crate::session::crop::CropTarget;
use es_anno::Viewport;
use es_anno::crop::{CropHandle, CropState};
use es_geom::{Affine, BezPath, FillRule, Point, Rgba};
use gpui::{
    App, Bounds, Context, CursorStyle, DispatchPhase, Entity, Hitbox, Hsla, KeyDownEvent,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window,
};
use kurbo::{Rect, RoundedRect, Shape, Vec2};

pub const HANDLE_HIT: f64 = 30.0;
const CORNER: f64 = 13.0;
const CORNER_RADIUS: f64 = 2.5;
const EDGE_LONG: f64 = 26.0;
const EDGE_SHORT: f64 = 7.0;
const EDGE_RADIUS: f64 = 3.0;
const BORDER: f64 = 1.5;
const THIRDS: f64 = 0.75;
const HANDLE_BORDER: f64 = 0.5;
const DIM: f32 = 0.55;

fn white(a: f32) -> Hsla {
    es_ui::convert::hsla(Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a,
    })
}

fn black(a: f32) -> Hsla {
    es_ui::convert::hsla(Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a,
    })
}

/// The crop rect in the canvas element's pixels.
pub fn screen_rect(state: &CropState, vp: &Viewport) -> Rect {
    let r = state.rect();
    Rect::from_points(
        vp.page_to_screen(Point::new(r.x0, r.y0)),
        vp.page_to_screen(Point::new(r.x1, r.y1)),
    )
}

pub fn handle_point(rect: Rect, handle: CropHandle) -> Point {
    let u = handle.unit_point();
    Point::new(rect.x0 + u.x * rect.width(), rect.y0 + u.y * rect.height())
}

/// Corners win over edges, and handles over the move surface; `None` is outside the crop.
pub fn hit(state: &CropState, vp: &Viewport, at: Point) -> Option<CropTarget> {
    let rect = screen_rect(state, vp);
    let near = |h: CropHandle| {
        let p = handle_point(rect, h);
        (at.x - p.x).abs() <= HANDLE_HIT / 2.0 && (at.y - p.y).abs() <= HANDLE_HIT / 2.0
    };
    let mut handles = CropHandle::ALL;
    handles.sort_by_key(|h| !h.is_corner());
    handles
        .into_iter()
        .filter(|h| h.visible(state.aspect()))
        .find(|h| near(*h))
        .map(CropTarget::Handle)
        .or_else(|| rect.contains(at).then_some(CropTarget::Move))
}

pub fn cursor_for(target: Option<CropTarget>, dragging: bool) -> CursorStyle {
    match target {
        None => CursorStyle::Arrow,
        Some(CropTarget::Move) if dragging => CursorStyle::ClosedHand,
        Some(CropTarget::Move) => CursorStyle::OpenHand,
        Some(CropTarget::Handle(h)) => match h {
            CropHandle::TopLeft | CropHandle::BottomRight => CursorStyle::ResizeUpLeftDownRight,
            CropHandle::TopRight | CropHandle::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            CropHandle::Top | CropHandle::Bottom => CursorStyle::ResizeUpDown,
            CropHandle::Left | CropHandle::Right => CursorStyle::ResizeLeftRight,
        },
    }
}

fn handle_shape(rect: Rect, handle: CropHandle) -> RoundedRect {
    let p = handle_point(rect, handle);
    let (w, h, radius) = match handle {
        _ if handle.is_corner() => (CORNER, CORNER, CORNER_RADIUS),
        CropHandle::Top | CropHandle::Bottom => (EDGE_LONG, EDGE_SHORT, EDGE_RADIUS),
        _ => (EDGE_SHORT, EDGE_LONG, EDGE_RADIUS),
    };
    Rect::from_center_size(p, (w, h)).to_rounded_rect(radius)
}

struct Pen {
    to_window: Affine,
    bucket: Bucket,
    alpha: f32,
    out: Vec<Painted>,
}

impl Pen {
    fn fill(&mut self, path: &BezPath, color: Hsla) {
        paths::fill(
            path,
            self.to_window,
            FillRule::NonZero,
            color.opacity(self.alpha),
            self.bucket,
            &mut self.out,
        );
    }

    fn stroke(&mut self, path: &BezPath, width: f64, color: Hsla) {
        let style = kurbo::Stroke::new(width);
        let outline = kurbo::stroke(path, &style, &kurbo::StrokeOpts::default(), 0.05);
        self.fill(&outline, color);
    }
}

/// Dimmed exterior, framed rect with thirds, and the handles, in window pixels; `rect` is the
/// crop as drawn, in image pixels, which differs from the state's while it moves.
pub fn overlay(
    state: &CropState,
    rect: Rect,
    alpha: f32,
    vp: &Viewport,
    bounds: Bounds<Pixels>,
    scale_factor: f32,
) -> Vec<Painted> {
    let mut pen = Pen {
        to_window: Affine::translate((f64::from(bounds.origin.x), f64::from(bounds.origin.y))),
        bucket: Bucket {
            index: 0,
            scale_factor,
        },
        alpha,
        out: Vec::new(),
    };
    let size = state.image_size();
    let image = Rect::from_points(
        vp.page_to_screen(Point::ZERO),
        vp.page_to_screen(Point::new(size.width, size.height)),
    );
    let crop = Rect::from_points(
        vp.page_to_screen(Point::new(rect.x0, rect.y0)),
        vp.page_to_screen(Point::new(rect.x1, rect.y1)),
    );
    let mut dim = BezPath::new();
    for band in [
        Rect::new(image.x0, image.y0, image.x1, crop.y0),
        Rect::new(image.x0, crop.y1, image.x1, image.y1),
        Rect::new(image.x0, crop.y0, crop.x0, crop.y1),
        Rect::new(crop.x1, crop.y0, image.x1, crop.y1),
    ] {
        if band.width() > 0.0 && band.height() > 0.0 {
            dim.extend(band.to_path(0.05));
        }
    }
    pen.fill(&dim, black(DIM));

    let mut thirds = BezPath::new();
    for i in [1.0, 2.0] {
        let x = crop.x0 + crop.width() * i / 3.0;
        let y = crop.y0 + crop.height() * i / 3.0;
        thirds
            .extend(Rect::new(x - THIRDS / 2.0, crop.y0, x + THIRDS / 2.0, crop.y1).to_path(0.05));
        thirds
            .extend(Rect::new(crop.x0, y - THIRDS / 2.0, crop.x1, y + THIRDS / 2.0).to_path(0.05));
    }
    pen.fill(&thirds, white(0.35));

    let frame = crop.inset(-BORDER / 2.0).to_path(0.05);
    pen.stroke(&frame, BORDER + 2.0, black(0.3));
    pen.stroke(&frame, BORDER, white(0.95));

    for handle in CropHandle::ALL
        .into_iter()
        .filter(|h| h.visible(state.aspect()))
    {
        let shape = handle_shape(crop, handle);
        let shadow = RoundedRect::from_rect(
            shape.rect().inflate(0.75, 0.75) + Vec2::new(0.0, 0.5),
            shape.radii().top_left + 0.75,
        );
        pen.fill(&shadow.to_path(0.05), black(0.22));
        let path = shape.to_path(0.05);
        pen.fill(&path, white(1.0));
        pen.stroke(&path, HANDLE_BORDER, black(0.18));
    }
    pen.out
}

impl EditorCanvas {
    pub(super) fn crop_overlay(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Painted> {
        let vp = self.viewport.read(cx).viewport();
        self.crop_shown(window, cx)
            .map(|s| {
                overlay(
                    &s.state,
                    s.rect,
                    s.alpha,
                    &vp,
                    bounds,
                    window.scale_factor(),
                )
            })
            .unwrap_or_default()
    }

    pub(super) fn is_cropping(&self, cx: &App) -> bool {
        self.crop.read(cx).is_cropping()
    }

    fn crop_target(&self, position: gpui::Point<Pixels>, cx: &App) -> Option<CropTarget> {
        let vp = self.viewport.read(cx).viewport();
        let crop = self.crop.read(cx);
        crop.dragging()
            .or_else(|| crop.state().and_then(|s| hit(s, &vp, self.local(position))))
    }

    fn crop_cursor(&self, position: gpui::Point<Pixels>, cx: &App) -> CursorStyle {
        let dragging = self.crop.read(cx).dragging().is_some();
        cursor_for(self.crop_target(position, cx), dragging)
    }

    fn page_at(&self, position: gpui::Point<Pixels>, cx: &App) -> Point {
        self.viewport
            .read(cx)
            .viewport()
            .screen_to_page(self.local(position))
    }

    fn crop_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_panning() {
            return;
        }
        window.focus(&self.focus, cx);
        if let Some(target) = self.crop_target(e.position, cx) {
            let at = self.page_at(e.position, cx);
            self.crop.update(cx, |c, _| c.press(target, at));
            cx.notify();
        }
    }

    fn crop_move(
        &mut self,
        position: gpui::Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let at = self.page_at(position, cx);
        let from_center = modifiers.alt || (modifiers.control && modifiers.shift);
        self.crop.update(cx, |c, cx| c.drag_to(at, from_center, cx));
    }

    /// Enter applies and Escape or C cancels; every other key is swallowed while cropping.
    pub(super) fn crop_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !self.is_cropping(cx) {
            return false;
        }
        let k = &e.keystroke;
        let plain = !(k.modifiers.control || k.modifiers.alt || k.modifiers.platform);
        if !e.is_held {
            match k.key.as_str() {
                "enter" => {
                    self.crop.update(cx, |c, cx| c.apply(cx));
                }
                "escape" => self.crop.update(cx, |c, cx| c.cancel(cx)),
                "c" if plain => self.crop.update(cx, |c, cx| c.cancel(cx)),
                _ => {}
            }
        }
        true
    }
}

/// Crop mode's pointer routing: the engine sees nothing while cropping.
pub(super) fn register(
    view: &Entity<EditorCanvas>,
    hitbox: &Hitbox,
    window: &mut Window,
    cx: &mut App,
) {
    let position = window.mouse_position();
    let style = view.update(cx, |c, cx| {
        c.cursor = c.crop_cursor(position, cx);
        c.cursor
    });
    window.set_cursor_style(style, hitbox);

    let (v, hb) = (view.clone(), hitbox.clone());
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && e.button == MouseButton::Left && hb.is_hovered(window)
        {
            v.update(cx, |c, cx| c.crop_down(e, window, cx));
        }
    });
    let (v, hb) = (view.clone(), hitbox.clone());
    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
        let dragging = v.read(cx).crop.read(cx).dragging().is_some();
        if dragging && phase == DispatchPhase::Capture {
            v.update(cx, |c, cx| c.crop_move(e.position, e.modifiers, cx));
        } else if !dragging && phase == DispatchPhase::Bubble && hb.is_hovered(window) {
            v.update(cx, |c, cx| {
                if c.crop_cursor(e.position, cx) != c.cursor {
                    cx.notify();
                }
            });
        }
    });
    let v = view.clone();
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture && e.button == MouseButton::Left {
            v.update(cx, |c, cx| {
                c.crop.update(cx, |crop, cx| crop.release(cx));
            });
        }
    });
}
