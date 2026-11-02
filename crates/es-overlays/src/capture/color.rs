use super::frozen::{Tile, tiles};
use super::paint::{self, b, black, white};
use super::strings::fl;
use super::window::local;
use es_geom::{Point, Rect, Rgba};
use es_model::platform::Frame;
use es_ui::a11y::{Accessible as _, Role};
use gpui::{
    App, Context, Corners, CursorStyle, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, div, fill, hsla, px, quad,
};
use std::sync::Arc;

pub const GRID: i64 = 9;
pub const LOUPE: f64 = 126.0;
const RING: f64 = 3.0;
const LABEL_GAP: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorEvent {
    Picked(Rgba),
    Cancel,
}

/// The frame pixel under a logical point relative to the output's top-left.
pub fn pixel_at(frame: &Frame, p: Point) -> Option<(i64, i64)> {
    let (w, h) = frame.size_px;
    let (lw, lh) = (frame.logical.width(), frame.logical.height());
    if lw <= 0.0 || lh <= 0.0 {
        return None;
    }
    let x = (p.x * f64::from(w) / lw).floor() as i64;
    let y = (p.y * f64::from(h) / lh).floor() as i64;
    (x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h)).then_some((x, y))
}

pub fn sample(frame: &Frame, (x, y): (i64, i64)) -> Option<[u8; 3]> {
    let (w, h) = frame.size_px;
    if x < 0 || y < 0 || x >= i64::from(w) || y >= i64::from(h) {
        return None;
    }
    let i = (y as usize * w as usize + x as usize) * 4;
    let px = frame.bgra.get(i..i + 3)?;
    Some([px[2], px[1], px[0]])
}

pub fn rgba([r, g, b]: [u8; 3]) -> Rgba {
    let c = |v: u8| f32::from(v) / 255.0;
    Rgba::new(c(r), c(g), c(b), 1.0)
}

pub fn hex(c: Rgba) -> String {
    let v = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", v(c.r), v(c.g), v(c.b))
}

pub struct ColorPicker {
    frame: Arc<Frame>,
    tiles: Arc<[Tile]>,
    bounds: Rect,
    pointer: Option<Point>,
    cursor: CursorStyle,
    focus: FocusHandle,
    focused: bool,
}

impl EventEmitter<ColorEvent> for ColorPicker {}

impl Focusable for ColorPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ColorPicker {
    pub fn new(frame: Arc<Frame>, cx: &mut Context<Self>) -> Self {
        ColorPicker {
            tiles: tiles(&frame).into(),
            bounds: Rect::from_origin_size((0.0, 0.0), frame.logical.size()),
            frame,
            pointer: None,
            cursor: paint::hidden_cursor(cx),
            focus: cx.focus_handle(),
            focused: false,
        }
    }

    pub fn color_at(&self, p: Point) -> Option<Rgba> {
        pixel_at(&self.frame, p)
            .and_then(|px| sample(&self.frame, px))
            .map(rgba)
    }

    fn moved(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        self.pointer = Some(local(e.position));
        cx.notify();
    }

    fn press(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        let p = local(e.position);
        self.pointer = Some(p);
        if let Some(c) = self.color_at(p) {
            cx.emit(ColorEvent::Picked(c));
        }
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let step = if e.keystroke.modifiers.shift { 10 } else { 1 };
        let (dx, dy) = match e.keystroke.key.as_str() {
            "escape" => return cx.emit(ColorEvent::Cancel),
            "enter" | "space" => {
                if let Some(c) = self.pointer.and_then(|p| self.color_at(p)) {
                    cx.emit(ColorEvent::Picked(c));
                }
                return;
            }
            "left" => (-step, 0),
            "right" => (step, 0),
            "up" => (0, -step),
            "down" => (0, step),
            _ => return,
        };
        self.nudge(dx, dy);
        cx.notify();
    }

    fn nudge(&mut self, dx: i64, dy: i64) {
        let (w, h) = self.frame.size_px;
        let start = self.pointer.unwrap_or(self.bounds.center());
        let Some((x, y)) = pixel_at(&self.frame, start) else {
            return;
        };
        let x = (x + dx).clamp(0, i64::from(w) - 1) as f64;
        let y = (y + dy).clamp(0, i64::from(h) - 1) as f64;
        let (lw, lh) = (self.frame.logical.width(), self.frame.logical.height());
        self.pointer = Some(Point::new(
            (x + 0.5) * lw / f64::from(w),
            (y + 0.5) * lh / f64::from(h),
        ));
    }

    fn loupe(&self) -> Option<Loupe> {
        let p = self.pointer?;
        let centre = pixel_at(&self.frame, p)?;
        let half = GRID / 2;
        let cells = (0..GRID * GRID)
            .map(|k| {
                sample(
                    &self.frame,
                    (centre.0 + k % GRID - half, centre.1 + k / GRID - half),
                )
            })
            .collect();
        let label = sample(&self.frame, centre).map(|c| SharedString::from(hex(rgba(c))));
        Some(Loupe {
            centre: p,
            cells,
            label,
        })
    }
}

struct Loupe {
    centre: Point,
    /// Row by row, top-left first; None past the frame's edge.
    cells: Vec<Option<[u8; 3]>>,
    label: Option<SharedString>,
}

impl Loupe {
    // A rounded image would blur the cells, so the circle is built from device-pixel rows of cell spans.
    fn paint(self, all: Rect, window: &mut Window, cx: &mut App) {
        let sf = f64::from(window.scale_factor());
        let d = (LOUPE * sf).round();
        let ox = ((self.centre.x - LOUPE / 2.) * sf).round();
        let oy = ((self.centre.y - LOUPE / 2.) * sf).round();
        let edge = |k: i64| (k as f64 * d / GRID as f64).round();
        let radius = d / 2.;
        let colour = |c: Option<[u8; 3]>| match c {
            Some([r, g, b]) => gpui::Rgba {
                r: f32::from(r) / 255.,
                g: f32::from(g) / 255.,
                b: f32::from(b) / 255.,
                a: 1.,
            }
            .into(),
            None => hsla(0., 0., 0.5, 1.),
        };
        let at = |x0: f64, y0: f64, x1: f64, y1: f64| {
            Rect::new(
                (ox + x0) / sf,
                (oy + y0) / sf,
                (ox + x1) / sf,
                (oy + y1) / sf,
            )
        };
        for row in 0..GRID {
            let (y0, y1) = (edge(row), edge(row + 1));
            let mut y = y0;
            while y < y1 {
                let dy = (y + 0.5 - radius).abs();
                let chord = (radius * radius - dy * dy).max(0.0).sqrt();
                let (xs, xe) = ((radius - chord).round(), (radius + chord).round());
                for col in 0..GRID {
                    let (x0, x1) = (edge(col).max(xs), edge(col + 1).min(xe));
                    if x1 > x0 {
                        let c = self.cells[(row * GRID + col) as usize];
                        window.paint_quad(fill(b(at(x0, y, x1, y + 1.)), colour(c)));
                    }
                }
                y += 1.;
            }
        }
        let circle = at(0., 0., d, d);
        let round = |r: Rect| Corners::all(px((r.width() / 2.) as f32));
        for (r, width, colour) in [
            (circle.inflate(1., 1.), RING, white(1.)),
            (circle.inflate(1.5, 1.5), 0.5, black(0.35)),
        ] {
            window.paint_quad(quad(
                b(r),
                round(r),
                gpui::transparent_black(),
                px(width as f32),
                colour,
                Default::default(),
            ));
        }
        let mid = GRID / 2;
        let cell = at(edge(mid), edge(mid), edge(mid + 1), edge(mid + 1));
        window.paint_quad(quad(
            b(cell.inflate(1., 1.)),
            Corners::default(),
            gpui::transparent_black(),
            px(1.),
            black(0.6),
            Default::default(),
        ));
        window.paint_quad(quad(
            b(cell),
            Corners::default(),
            gpui::transparent_black(),
            px(1.5),
            white(1.),
            Default::default(),
        ));
        if let Some(text) = self.label {
            let below = (self.centre.x, circle.y1 + 1.5 + LABEL_GAP);
            paint::pill(text, below, true, all.inset(-4.), black(0.7), window, cx);
        }
    }
}

impl Render for ColorPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let (tiles, all) = (self.tiles.clone(), self.bounds);
        let loupe = self.loupe();
        let described = loupe
            .as_ref()
            .and_then(|l| l.label.clone())
            .unwrap_or_default();
        let (w, h) = (all.width() as f32, all.height() as f32);
        div()
            .id("color-picker")
            .accessible(Role::Canvas, fl!("color-a11y"))
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
                    move |_, _, window, cx| {
                        paint::tiles(&tiles, window);
                        if let Some(loupe) = loupe {
                            loupe.paint(all, window, cx);
                        }
                    },
                )
                .size_full(),
            )
    }
}
