use crate::capture::strings::fl;
use crate::deck::card::{black, white};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SurfaceHandle as _, SurfaceRole,
};
use es_geom::{Point, Rect, Size};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::icons::IconName;
use es_ui::motion::{Easing, Transition, animate};
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    App, AppContext as _, BoxShadow, Context, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit,
    ParentElement as _, Pixels, Render, RenderImage, ScrollDelta, ScrollEvent,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription, Window, canvas,
    div, img, point, px,
};
use gpui_component::Icon;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub const MAX_SIDE: f64 = 400.;
pub const MIN_SIZE: Size = Size::new(80., 60.);
pub const CASCADE: f64 = 20.;
pub const SCALE_STEP: f64 = 0.05;
pub const MIN_SCALE: f64 = 0.25;
pub const MAX_SCALE: f64 = 4.;
pub const MIN_OPACITY: f32 = 0.2;
const OPACITY_STEP: f32 = 0.05;
/// Room around the picture for its shadows; the surface is this much larger on every side.
pub const SHADOW_PAD: f64 = 16.;
const LINE: f32 = 20.;
const RADIUS: Pixels = px(8.);
const CLOSE: f32 = 18.;
const CLOSE_PAD: f32 = 4.;
const CLOSE_FADE: Transition = Transition {
    duration: Duration::from_millis(150),
    easing: Easing::EaseOut,
    fade: true,
};

/// The longest side at most 400, never enlarged, and never smaller than 80 × 60.
pub fn initial_size(image: Size) -> Size {
    let longest = image.width.max(image.height);
    let fit = if longest > MAX_SIDE {
        MAX_SIDE / longest
    } else {
        1.
    };
    Size::new(
        (image.width * fit).max(MIN_SIZE.width),
        (image.height * fit).max(MIN_SIZE.height),
    )
}

/// Centred on the output; each pin already open moves the next one 20 right and 20 down.
pub fn initial_origin(output: Rect, size: Size, open: usize) -> Point {
    let step = CASCADE * open as f64;
    Point::new(
        output.center().x - size.width / 2. + step,
        output.center().y - size.height / 2. + step,
    )
}

pub fn next_scale(scale: f64, lines: f64) -> f64 {
    (scale + lines * SCALE_STEP).clamp(MIN_SCALE, MAX_SCALE)
}

pub fn scaled(initial: Size, scale: f64) -> Size {
    Size::new(initial.width * scale, initial.height * scale)
}

/// Resizing keeps the top-left corner where it is.
pub fn anchored(frame: Rect, size: Size) -> Rect {
    Rect::from_origin_size(frame.origin(), size)
}

/// From the global pointer, never from surface-local deltas, which lag the surface's own moves.
pub fn dragged(press_origin: Point, press_pointer: Point, pointer: Point) -> Point {
    press_origin + (pointer - press_pointer)
}

pub fn next_opacity(opacity: f32, lines: f32) -> f32 {
    (opacity + lines * OPACITY_STEP).clamp(MIN_OPACITY, 1.)
}

fn lines(delta: ScrollDelta) -> f32 {
    match delta {
        ScrollDelta::Lines(l) => l.y,
        ScrollDelta::Pixels(p) => f32::from(p.y) / LINE,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinEvent {
    /// The picture's new top-left, in global logical pixels.
    Moved(Point),
    Resized(Size),
    CopyImage,
    Close,
}

#[derive(Clone, Copy)]
struct Press {
    pointer: Point,
    origin: Point,
}

pub struct PinView {
    image: Arc<RenderImage>,
    initial: Size,
    scale: f64,
    origin: Point,
    shown_at: Rc<Cell<Point>>,
    opacity: f32,
    hovered: bool,
    showings: usize,
    menu: Option<gpui::Point<Pixels>>,
    press: Option<Press>,
}

impl EventEmitter<PinEvent> for PinView {}

impl PinView {
    pub fn new(image: Arc<RenderImage>, initial: Size, origin: Point) -> Self {
        PinView {
            image,
            initial,
            scale: 1.,
            origin,
            shown_at: Rc::new(Cell::new(origin)),
            opacity: 1.,
            hovered: false,
            showings: 0,
            menu: None,
            press: None,
        }
    }

    pub fn size(&self) -> Size {
        scaled(self.initial, self.scale)
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Where the pointer is on the output, from where the surface was when it was last drawn.
    fn global(&self, local: gpui::Point<Pixels>) -> Point {
        let at = self.shown_at.get();
        Point::new(
            at.x - SHADOW_PAD + f64::from(f32::from(local.x)),
            at.y - SHADOW_PAD + f64::from(f32::from(local.y)),
        )
    }

    fn mouse_down(&mut self, e: &MouseDownEvent, cx: &mut Context<Self>) {
        match e.button {
            MouseButton::Left => {
                self.menu = None;
                self.press = Some(Press {
                    pointer: self.global(e.position),
                    origin: self.origin,
                });
            }
            MouseButton::Right => self.menu = Some(e.position),
            _ => return,
        }
        cx.notify();
    }

    fn mouse_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(press) = self.press else {
            return;
        };
        if e.pressed_button != Some(MouseButton::Left) {
            self.press = None;
            return;
        }
        let origin = dragged(press.origin, press.pointer, self.global(e.position));
        if origin != self.origin {
            self.origin = origin;
            cx.emit(PinEvent::Moved(origin));
            cx.notify();
        }
    }

    fn scroll(&mut self, delta: ScrollDelta, modifiers: Modifiers, cx: &mut Context<Self>) {
        let lines = lines(delta);
        if modifiers.control {
            self.opacity = next_opacity(self.opacity, lines);
            cx.notify();
            return;
        }
        let scale = next_scale(self.scale, f64::from(lines));
        if scale != self.scale {
            self.scale = scale;
            cx.emit(PinEvent::Resized(self.size()));
            cx.notify();
        }
    }

    fn menu(&self, at: gpui::Point<Pixels>, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let t = tokens(cx);
        let item = |id: &'static str, label: String, event: PinEvent| {
            div()
                .id(id)
                .accessible(Role::MenuItem, label.clone())
                .px(px(10.))
                .py(px(3.))
                .rounded(px(4.))
                .hover(|s| s.bg(t.colours.hover_fill))
                .child(label)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.menu = None;
                    cx.emit(event);
                    cx.notify();
                }))
        };
        let size = self.size();
        let x = f32::from(at.x)
            .min((SHADOW_PAD + size.width) as f32 - 104.)
            .max(SHADOW_PAD as f32);
        let y = f32::from(at.y)
            .min((SHADOW_PAD + size.height) as f32 - 58.)
            .max(SHADOW_PAD as f32);
        div()
            .id("pin-menu")
            .accessible(Role::Menu, fl!("pin-a11y"))
            .occlude()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(104.))
            .p(px(4.))
            .rounded(t.metrics.popover_radius)
            .border(t.metrics.hairline)
            .border_color(t.colours.border)
            .bg(t.colours.dropdown)
            .shadow(vec![shadow(0.25, 10., 4.)])
            .font_family(UI_FONT)
            .text_size(px(12.))
            .text_color(t.colours.text)
            .child(item("pin-copy", fl!("pin-copy-image"), PinEvent::CopyImage))
            .child(item("pin-close", fl!("pin-close"), PinEvent::Close))
    }
}

fn shadow(alpha: f32, blur: f32, y: f32) -> BoxShadow {
    BoxShadow {
        color: black(alpha),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
        inset: false,
    }
}

impl Render for PinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.size();
        let pad = px(SHADOW_PAD as f32);
        let view = cx.entity().downgrade();
        let shown_at = self.shown_at.clone();
        let drawn = self.origin;
        let close = self.hovered.then(|| {
            animate(
                div()
                    .id("pin-close-button")
                    .accessible(Role::Button, fl!("pin-close"))
                    .absolute()
                    .top(px(CLOSE_PAD))
                    .right(px(CLOSE_PAD))
                    .size(px(CLOSE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(black(0.6))
                    .cursor_pointer()
                    .child(Icon::new(IconName::X).size(px(11.)).text_color(white(1.)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(PinEvent::Close))),
                ("pin-close-fade", self.showings),
                CLOSE_FADE,
                cx,
                |e, t| e.opacity(t),
            )
        });
        let menu = self.menu.map(|at| self.menu(at, cx));
        div()
            .size_full()
            .relative()
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        // The surface's margins change with the frame after a move, so this frame shows `drawn`.
                        shown_at.set(drawn);
                        let moving = view.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, _, _, cx| {
                            let _ = moving.update(cx, |v, cx| v.mouse_move(e, cx));
                        });
                        let releasing = view.clone();
                        window.on_mouse_event(move |e: &MouseUpEvent, _, _, cx| {
                            if e.button == MouseButton::Left {
                                let _ = releasing.update(cx, |v, _| v.press = None);
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .id("pin")
                    .accessible(Role::Image, fl!("pin-a11y"))
                    .absolute()
                    .left(pad)
                    .top(pad)
                    .w(px(size.width as f32))
                    .h(px(size.height as f32))
                    .rounded(RADIUS)
                    .shadow(vec![shadow(0.4, 12., 4.), shadow(0.15, 4., 2.)])
                    .opacity(self.opacity)
                    .on_hover(cx.listener(|v, hovered: &bool, _, cx| {
                        if v.hovered != *hovered {
                            v.hovered = *hovered;
                            v.showings += usize::from(*hovered);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|v, e, _, cx| v.mouse_down(e, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|v, e, _, cx| v.mouse_down(e, cx)),
                    )
                    .on_scroll(
                        cx.listener(|v, e: &ScrollEvent, _, cx| v.scroll(e.delta, e.modifiers, cx)),
                    )
                    .child(
                        div().size_full().rounded(RADIUS).overflow_hidden().child(
                            img(self.image.clone())
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                    )
                    .children(close),
            )
            .children(menu)
    }
}

fn surface_size(size: Size) -> Size {
    Size::new(size.width + 2. * SHADOW_PAD, size.height + 2. * SHADOW_PAD)
}

fn surface_origin(origin: Point) -> Point {
    Point::new(origin.x - SHADOW_PAD, origin.y - SHADOW_PAD)
}

fn picture_region(size: Size) -> Rect {
    Rect::from_origin_size(Point::new(SHADOW_PAD, SHADOW_PAD), size)
}

struct Pinned {
    id: u64,
    handle: LayerSurfaceHandle,
    view: Entity<PinView>,
    _events: Subscription,
}

pub type CopyImage = Rc<dyn Fn(&Path, &mut App)>;

/// The pinned screenshots on screen; they are not kept across restarts.
pub struct Pins {
    surfaces: Rc<LayerShellSurfaces>,
    pinned: Rc<RefCell<Vec<Pinned>>>,
    copy: CopyImage,
    next: Cell<u64>,
}

impl Pins {
    pub fn new(surfaces: Rc<LayerShellSurfaces>, copy: CopyImage) -> Self {
        Pins {
            surfaces,
            pinned: Rc::default(),
            copy,
            next: Cell::new(1),
        }
    }

    pub fn count(&self) -> usize {
        self.pinned.borrow().len()
    }

    pub fn views(&self) -> Vec<Entity<PinView>> {
        self.pinned
            .borrow()
            .iter()
            .map(|p| p.view.clone())
            .collect()
    }

    /// `image` is in pixels; the pin shows it at the output's scale.
    pub fn pin(
        &self,
        file: PathBuf,
        image: Arc<RenderImage>,
        cx: &mut App,
    ) -> Result<Entity<PinView>> {
        let output = self
            .surfaces
            .test_output()
            .cloned()
            .or_else(|| self.surfaces.source().pointer_output())
            .or_else(|| self.surfaces.source().focused_output())
            .ok_or_else(|| Error::NotFound("an output for the pin".into()))?;
        let info = self.surfaces.output_info(&output)?;
        let px = image.size(0);
        let scale = info.scale.max(1.);
        let natural = Size::new(
            f64::from(i32::from(px.width)) / scale,
            f64::from(i32::from(px.height)) / scale,
        );
        let size = initial_size(natural);
        let origin = initial_origin(info.logical, size, self.count());
        let handle =
            self.surfaces
                .open_surface(SurfaceRole::Pin, &output, surface_size(size), cx)?;
        let view = cx.new(|_| PinView::new(image, size, origin));
        handle.set_content(view.clone().into(), cx);
        handle.set_position(surface_origin(origin), cx);
        handle.set_input_region(&[picture_region(size)], cx);
        let id = self.next.get();
        self.next.set(id + 1);
        let (pinned, copy) = (Rc::downgrade(&self.pinned), self.copy.clone());
        let events = cx.subscribe(&view, move |_, e: &PinEvent, cx| {
            let Some(pinned) = pinned.upgrade() else {
                return;
            };
            match *e {
                PinEvent::CopyImage => copy(&file, cx),
                PinEvent::Close => close(&pinned, id, cx),
                PinEvent::Moved(origin) => {
                    if let Some(p) = pinned.borrow().iter().find(|p| p.id == id) {
                        p.handle.set_position(surface_origin(origin), cx);
                    }
                }
                PinEvent::Resized(size) => {
                    let list = pinned.borrow();
                    if let Some(p) = list.iter().find(|p| p.id == id) {
                        let origin = p.view.read(cx).origin();
                        p.handle.resize(surface_size(size), cx);
                        p.handle.set_position(surface_origin(origin), cx);
                        p.handle.set_input_region(&[picture_region(size)], cx);
                    }
                }
            }
        });
        self.pinned.borrow_mut().push(Pinned {
            id,
            handle,
            view: view.clone(),
            _events: events,
        });
        Ok(view)
    }

    pub fn unpin_all(&self, cx: &mut App) {
        let all = std::mem::take(&mut *self.pinned.borrow_mut());
        for p in all {
            p.handle.close(cx);
        }
    }
}

fn close(pinned: &RefCell<Vec<Pinned>>, id: u64, cx: &mut App) {
    let gone = {
        let mut list = pinned.borrow_mut();
        list.iter().position(|p| p.id == id).map(|i| list.remove(i))
    };
    if let Some(p) = gone {
        p.handle.close(cx);
    }
}
