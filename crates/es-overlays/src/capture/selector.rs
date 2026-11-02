use super::frozen::{Tile, tiles};
use super::geometry::{self, Handle};
use super::strings::fl;
use es_geom::{Point, Rect};
use es_model::platform::Frame;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::theme::UI_FONT;
use gpui::{
    App, Bounds, Context, Corners, CursorStyle, EventEmitter, FocusHandle, Focusable, Font,
    FontFeatures, FontWeight, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, TextAlign, TextRun, Window, canvas,
    div, fill, hsla, point, px, quad,
};
use std::sync::Arc;

const DIM: f32 = 0.3;
const BORDER: f32 = 1.5;
const HANDLE_DIAMETER: f32 = 8.0;
const GUIDE: f32 = 0.5;
const LABEL_SIZE: f32 = 11.0;
const LABEL_PAD_X: f32 = 6.0;
const LABEL_PAD_Y: f32 = 2.0;
const LABEL_GAP: f32 = 4.0;
const LABEL_INSET: f32 = 8.0;
// The live hole stays hidden until the drag is bigger than a click's jitter.
const LIVE_MIN: f64 = 2.0;

fn white(a: f32) -> Hsla {
    hsla(0., 0., 1., a)
}

fn black(a: f32) -> Hsla {
    hsla(0., 0., 0., a)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectorEvent {
    /// A press started a new selection here, so other outputs drop theirs.
    Began,
    /// Logical, relative to the output's top-left.
    Capture(Rect),
    Cancel,
    WindowMode,
}

#[derive(Debug, Clone, Copy)]
enum Gesture {
    Draw {
        start: Point,
        current: Point,
    },
    Adjust {
        handle: Handle,
        origin: Point,
        base: Rect,
        cursor: CursorStyle,
    },
}

pub struct RegionSelector {
    frame: Arc<Frame>,
    tiles: Arc<[Tile]>,
    bounds: Rect,
    selection: Option<Rect>,
    movable: bool,
    gesture: Option<Gesture>,
    pointer: Option<Point>,
    capture_on_release: bool,
    focus: FocusHandle,
    focused: bool,
}

impl EventEmitter<SelectorEvent> for RegionSelector {}

impl Focusable for RegionSelector {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RegionSelector {
    /// A `preselect` area can be resized but not moved, so drawing over it still starts a new one.
    pub fn new(
        frame: Arc<Frame>,
        preselect: Option<Rect>,
        capture_on_release: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let bounds = Rect::from_origin_size((0.0, 0.0), frame.logical.size());
        let selection = preselect
            .map(|r| r.intersect(bounds))
            .filter(|r| geometry::counts(*r));
        RegionSelector {
            tiles: tiles(&frame).into(),
            frame,
            bounds,
            selection,
            movable: false,
            gesture: None,
            pointer: None,
            capture_on_release,
            focus: cx.focus_handle(),
            focused: false,
        }
    }

    pub fn selection(&self) -> Option<Rect> {
        self.selection
    }

    pub fn frame(&self) -> &Arc<Frame> {
        &self.frame
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.selection = None;
        self.gesture = None;
        cx.notify();
    }

    fn cursor(&self) -> CursorStyle {
        match self.gesture {
            Some(Gesture::Adjust { cursor, .. }) => cursor,
            Some(Gesture::Draw { .. }) => CursorStyle::Crosshair,
            None => match (self.selection, self.pointer) {
                (Some(sel), Some(p)) => match geometry::handle_at(p, sel, self.movable) {
                    Some(h) => hover_cursor(h),
                    None => CursorStyle::Crosshair,
                },
                _ => CursorStyle::Crosshair,
            },
        }
    }

    fn press(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let p = local(e.position);
        self.pointer = Some(p);
        if let Some(sel) = self.selection {
            if e.click_count == 2 && sel.contains(p) {
                cx.emit(SelectorEvent::Capture(sel));
                return;
            }
            if let Some(handle) = geometry::handle_at(p, sel, self.movable) {
                let cursor = match handle {
                    Handle::Move => CursorStyle::ClosedHand,
                    h => hover_cursor(h),
                };
                self.gesture = Some(Gesture::Adjust {
                    handle,
                    origin: p,
                    base: sel,
                    cursor,
                });
                cx.notify();
                return;
            }
        }
        self.gesture = Some(Gesture::Draw {
            start: p,
            current: p,
        });
        cx.emit(SelectorEvent::Began);
        cx.notify();
    }

    fn moved(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let p = local(e.position);
        self.pointer = Some(p);
        match &mut self.gesture {
            Some(Gesture::Adjust {
                handle,
                origin,
                base,
                ..
            }) => {
                self.selection = Some(geometry::apply(*handle, *origin, p, *base, self.bounds));
            }
            Some(Gesture::Draw { current, .. }) => *current = p,
            None => {}
        }
        cx.notify();
    }

    fn released(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        let p = local(e.position);
        self.pointer = Some(p);
        match self.gesture.take() {
            Some(Gesture::Draw { start, .. }) => {
                let rect = geometry::drawn(start, p, self.bounds);
                if geometry::counts(rect) {
                    if self.capture_on_release {
                        cx.emit(SelectorEvent::Capture(rect));
                    } else {
                        self.selection = Some(rect);
                        self.movable = true;
                    }
                } else if self.selection.is_none() {
                    cx.emit(SelectorEvent::Cancel);
                }
            }
            Some(Gesture::Adjust { .. }) | None => {}
        }
        cx.notify();
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "escape" => cx.emit(SelectorEvent::Cancel),
            "space" => cx.emit(SelectorEvent::WindowMode),
            "enter" | "kp_enter" => {
                if let Some(sel) = self.selection {
                    cx.emit(SelectorEvent::Capture(sel));
                }
            }
            _ => {}
        }
    }

    fn scene(&self) -> Scene {
        let px_of = |r: Rect| {
            geometry::to_pixels(r, self.bounds.size(), self.frame.size_px)
                .map_or((0, 0), |p| (p.w, p.h))
        };
        let (shown, adjustable) = match self.gesture {
            Some(Gesture::Draw { start, current }) => {
                let r = geometry::drawn(start, current, self.bounds);
                (
                    (r.width() > LIVE_MIN && r.height() > LIVE_MIN).then_some(r),
                    false,
                )
            }
            _ => (self.selection, self.selection.is_some()),
        };
        let label = shown.map(|r| {
            let (w, h) = px_of(r);
            let text = if adjustable {
                let hint = if self.movable {
                    fl!("region-hint-drag-to-adjust")
                } else {
                    fl!("region-hint-drag-edges")
                };
                fl!("region-adjust", w = w, h = h, shortcut = "↩", hint = hint)
            } else {
                fl!("region-size", w = w, h = h)
            };
            SharedString::from(text)
        });
        let guides = match (self.gesture, self.selection) {
            (None, None) => self.pointer,
            _ => None,
        };
        Scene {
            tiles: self.tiles.clone(),
            bounds: self.bounds,
            shown,
            handles: adjustable,
            label,
            guides,
        }
    }
}

fn local(p: gpui::Point<Pixels>) -> Point {
    Point::new(f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
}

fn hover_cursor(h: Handle) -> CursorStyle {
    match h {
        Handle::TopLeft | Handle::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        Handle::TopRight | Handle::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
        Handle::Top | Handle::Bottom => CursorStyle::ResizeUpDown,
        Handle::Left | Handle::Right => CursorStyle::ResizeLeftRight,
        Handle::Move => CursorStyle::OpenHand,
    }
}

struct Scene {
    tiles: Arc<[Tile]>,
    bounds: Rect,
    shown: Option<Rect>,
    handles: bool,
    label: Option<SharedString>,
    guides: Option<Point>,
}

fn b(r: Rect) -> Bounds<Pixels> {
    es_ui::convert::bounds(r)
}

impl Scene {
    fn paint(self, window: &mut Window, cx: &mut App) {
        for tile in self.tiles.iter() {
            let at = b(tile.local);
            if let Err(error) =
                window.paint_image(at, at, Corners::default(), tile.image.clone(), 0, false)
            {
                tracing::warn!(%error, "frozen tile not painted");
            }
        }
        let all = self.bounds;
        match self.shown {
            None => window.paint_quad(fill(b(all), black(DIM))),
            Some(r) => {
                let r = r.intersect(all);
                for strip in [
                    Rect::new(all.x0, all.y0, all.x1, r.y0),
                    Rect::new(all.x0, r.y1, all.x1, all.y1),
                    Rect::new(all.x0, r.y0, r.x0, r.y1),
                    Rect::new(r.x1, r.y0, all.x1, r.y1),
                ] {
                    if strip.width() > 0.0 && strip.height() > 0.0 {
                        window.paint_quad(fill(b(strip), black(DIM)));
                    }
                }
                let half = f64::from(BORDER) / 2.0;
                window.paint_quad(quad(
                    b(r.inflate(half, half)),
                    Corners::default(),
                    gpui::transparent_black(),
                    px(BORDER),
                    white(1.),
                    Default::default(),
                ));
                if self.handles {
                    let d = f64::from(HANDLE_DIAMETER);
                    for h in Handle::EDGES {
                        let c = h.point(r);
                        window.paint_quad(quad(
                            b(Rect::from_center_size(c, (d, d))),
                            Corners::all(px(HANDLE_DIAMETER / 2.)),
                            white(1.),
                            px(1.),
                            black(0.5),
                            Default::default(),
                        ));
                    }
                }
                if let Some(text) = self.label {
                    paint_label(text, r, all, window, cx);
                }
            }
        }
        if let Some(p) = self.guides {
            // At fractional scales a 0.5 pt line straddles pixels and snapping can erase it, so it covers whole pixels.
            let sf = f64::from(window.scale_factor());
            let thick = (f64::from(GUIDE) * sf).round().max(1.0) / sf;
            let (x, y) = ((p.x * sf).floor() / sf, (p.y * sf).floor() / sf);
            window.paint_quad(fill(b(Rect::new(all.x0, y, all.x1, y + thick)), white(0.4)));
            window.paint_quad(fill(b(Rect::new(x, all.y0, x + thick, all.y1)), white(0.4)));
        }
    }
}

fn paint_label(text: SharedString, r: Rect, all: Rect, window: &mut Window, cx: &mut App) {
    let font = Font {
        family: UI_FONT.into(),
        features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
        weight: FontWeight::MEDIUM,
        ..gpui::font(UI_FONT)
    };
    let run = TextRun {
        len: text.len(),
        font,
        color: white(1.),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let font_size = px(LABEL_SIZE);
    let line_height = px((LABEL_SIZE * 1.3).round());
    let line = window
        .text_system()
        .shape_line(text, font_size, &[run], None);
    let w = f64::from(f32::from(line.width) + LABEL_PAD_X * 2.);
    let h = f64::from(f32::from(line_height) + LABEL_PAD_Y * 2.);
    let inset = f64::from(LABEL_INSET);
    let x = ((r.x0 + r.x1) / 2. - w / 2.)
        .max(all.x0 + inset)
        .min(all.x1 - w - inset);
    let y = (r.y1 + f64::from(LABEL_GAP)).min(all.y1 - h - inset);
    let at = Rect::new(x, y, x + w, y + h);
    window.paint_quad(quad(
        b(at),
        Corners::all(px(4.)),
        black(0.7),
        px(0.),
        gpui::transparent_black(),
        Default::default(),
    ));
    let origin = point(
        px((x + f64::from(LABEL_PAD_X)) as f32),
        px((y + f64::from(LABEL_PAD_Y)) as f32),
    );
    if let Err(error) = line.paint(origin, line_height, TextAlign::Left, None, window, cx) {
        tracing::warn!(%error, "selection label not painted");
    }
}

impl Render for RegionSelector {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let scene = self.scene();
        let (w, h) = (self.bounds.width() as f32, self.bounds.height() as f32);
        let described = scene.label.clone().unwrap_or_default();
        div()
            .id("region-selector")
            .accessible(Role::Canvas, fl!("region-a11y"))
            .aria_description(described)
            .track_focus(&self.focus)
            .w(px(w))
            .h(px(h))
            .cursor(self.cursor())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.press(e, cx)),
            )
            .on_mouse_move(cx.listener(|this, e, _, cx| this.moved(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, e, _, cx| this.released(e, cx)),
            )
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
