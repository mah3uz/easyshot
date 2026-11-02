use super::frozen::{Tile, tiles};
use super::geometry;
use super::paint::{self, b};
use super::strings::fl;
use super::windows::{Candidate, hit};
use es_app::errors::{Error, Result};
use es_geom::{Point, Rect};
use es_model::platform::{Frame, WindowId};
use es_ui::a11y::{Accessible as _, Role};
use gpui::{
    App, Context, Corners, CursorStyle, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, px, quad,
};
use std::sync::Arc;

const OUTLINE: f32 = 2.0;
const TINT: f32 = 0.12;
const PILL_INSET: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PickEvent {
    /// `local` is the window's rectangle on this output, used to crop when it cannot be captured alone.
    Window {
        id: WindowId,
        local: Option<Rect>,
    },
    Cancel,
    Region,
}

pub fn describe(c: &Candidate) -> String {
    match (c.app_id.is_empty(), c.title.is_empty()) {
        (false, false) => fl!(
            "window-item",
            app = c.app_id.as_str(),
            title = c.title.as_str()
        ),
        (false, true) => c.app_id.clone(),
        (true, false) => c.title.clone(),
        (true, true) => fl!("window-untitled"),
    }
}

/// The window's own pixels when the compositor gives them, else its rectangle cut from the frozen frame.
pub fn window_frame<E: std::fmt::Display>(
    captured: std::result::Result<Frame, E>,
    frozen: &Frame,
    local: Option<Rect>,
) -> Result<Frame> {
    match captured {
        Ok(own) => Ok(own),
        Err(error) => {
            let local = local.ok_or_else(|| Error::Protocol(error.to_string()))?;
            tracing::info!(%error, "window cropped from the frozen frame");
            geometry::crop(frozen, local).ok_or_else(|| Error::Internal("empty window".into()))
        }
    }
}

pub(crate) fn local(p: gpui::Point<Pixels>) -> Point {
    Point::new(f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
}

pub struct WindowSelector {
    frame: Arc<Frame>,
    tiles: Arc<[Tile]>,
    bounds: Rect,
    candidates: Arc<[Candidate]>,
    hovered: Option<usize>,
    cursor: CursorStyle,
    focus: FocusHandle,
    focused: bool,
}

impl EventEmitter<PickEvent> for WindowSelector {}

impl Focusable for WindowSelector {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WindowSelector {
    /// `candidates` are front to back; the first one under the pointer is the one picked.
    pub fn new(frame: Arc<Frame>, candidates: Vec<Candidate>, cx: &mut Context<Self>) -> Self {
        WindowSelector {
            tiles: tiles(&frame).into(),
            bounds: Rect::from_origin_size((0.0, 0.0), frame.logical.size()),
            frame,
            candidates: candidates.into(),
            hovered: None,
            cursor: paint::camera_cursor(cx),
            focus: cx.focus_handle(),
            focused: false,
        }
    }

    pub fn frame(&self) -> &Arc<Frame> {
        &self.frame
    }

    pub fn hovered(&self) -> Option<&Candidate> {
        self.candidates.get(self.hovered?)
    }

    fn moved(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let now = hit(&self.candidates, local(e.position));
        if now != self.hovered {
            self.hovered = now;
            cx.notify();
        }
    }

    fn press(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        self.hovered = hit(&self.candidates, local(e.position));
        if let Some(c) = self.hovered() {
            cx.emit(PickEvent::Window {
                id: c.id.clone(),
                local: c.local,
            });
        }
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => cx.emit(PickEvent::Cancel),
            "space" => cx.emit(PickEvent::Region),
            _ => {}
        }
    }
}

struct Scene {
    tiles: Arc<[Tile]>,
    bounds: Rect,
    hovered: Option<(Rect, SharedString)>,
    accent: Hsla,
}

impl Scene {
    fn paint(self, window: &mut Window, cx: &mut App) {
        paint::tiles(&self.tiles, window);
        let Some((r, title)) = self.hovered else {
            return;
        };
        let accent = self.accent;
        window.paint_quad(fill(b(r), accent.opacity(TINT)));
        window.paint_quad(quad(
            b(r),
            Corners::default(),
            gpui::transparent_black(),
            px(OUTLINE),
            accent,
            Default::default(),
        ));
        let corner = r.intersect(self.bounds);
        let at = (corner.x0 + PILL_INSET, corner.y0 + PILL_INSET);
        paint::pill(
            title,
            at,
            false,
            self.bounds.inset(-PILL_INSET),
            accent,
            window,
            cx,
        );
    }
}

impl Render for WindowSelector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let hovered = self
            .hovered()
            .and_then(|c| Some((c.local?, SharedString::from(describe(c)))));
        let described = hovered.as_ref().map(|h| h.1.clone()).unwrap_or_default();
        let scene = Scene {
            tiles: self.tiles.clone(),
            bounds: self.bounds,
            hovered,
            accent: paint::accent(cx),
        };
        let (w, h) = (self.bounds.width() as f32, self.bounds.height() as f32);
        div()
            .id("window-selector")
            .accessible(Role::Canvas, fl!("window-a11y"))
            .aria_description(described)
            .track_focus(&self.focus)
            .w(px(w))
            .h(px(h))
            .cursor(self.cursor)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.press(e, cx)),
            )
            .on_mouse_move(cx.listener(|this, e, _, cx| this.moved(e, cx)))
            .on_key_down(cx.listener(|this, e, _, cx| this.key(e, cx)))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, cx| scene.paint(window, cx),
                )
                .size_full(),
            )
    }
}
