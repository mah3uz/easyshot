#![allow(clippy::unwrap_used)]
mod window_mode_support;

use es_geom::{Point, Rect, Size};
use es_overlays::pin::{
    CopyImage, MAX_SCALE, MIN_OPACITY, MIN_SCALE, PinEvent, PinView, SHADOW_PAD, anchored,
    next_scale, scaled,
};
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, PlatformInput, RenderImage, ScrollDelta, ScrollEvent,
    point, px,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use window_mode_support::{Surface, near, rgb};

const PICTURE: Size = Size::new(300., 200.);
const START: Point = Point::new(100., 100.);
const PAD: f32 = SHADOW_PAD as f32;

#[test]
fn zooming_keeps_the_top_left_corner_put() {
    let start = Rect::new(300., 500., 700., 750.);
    let bigger = anchored(start, Size::new(600., 375.));
    assert_eq!((bigger.x0, bigger.y0), (start.x0, start.y0));
    assert_eq!(bigger.size(), Size::new(600., 375.));
    let smaller = anchored(start, Size::new(200., 125.));
    assert_eq!((smaller.x0, smaller.y0), (start.x0, start.y0));
    assert_eq!(
        anchored(start, start.size()),
        start,
        "a no-op zoom stays put"
    );

    let grown = anchored(Rect::new(0., 0., 400., 250.), Size::new(800., 500.));
    assert_eq!((grown.y0, grown.y1), (0., 500.), "growing extends downward");
}

#[test]
fn scrolling_in_and_back_out_returns_to_the_start_without_drift() {
    let initial = Size::new(400., 250.);
    let start = Rect::from_origin_size(Point::new(300., 500.), initial);
    let (mut frame, mut scale) = (start, 1.);
    for lines in [1.; 40].into_iter().chain([-1.; 40]) {
        scale = next_scale(scale, lines);
        frame = anchored(frame, scaled(initial, scale));
    }
    assert!((frame.y0 - start.y0).abs() < 1e-9 && (frame.x0 - start.x0).abs() < 1e-9);
    assert!((frame.width() - initial.width).abs() < 1e-6, "{frame:?}");
    assert_eq!(next_scale(MAX_SCALE, 1.), MAX_SCALE);
    assert_eq!(next_scale(MIN_SCALE, -1.), MIN_SCALE);
}

fn same(a: Size, b: Size) -> bool {
    (a.width - b.width).abs() < 1e-6 && (a.height - b.height).abs() < 1e-6
}

fn picture() -> Arc<RenderImage> {
    let pixels = image::RgbaImage::from_pixel(
        PICTURE.width as u32,
        PICTURE.height as u32,
        image::Rgba([230, 120, 40, 255]),
    );
    Arc::new(RenderImage::new([image::Frame::new(pixels)]))
}

struct Rig {
    s: Surface<PinView>,
    events: Rc<RefCell<Vec<PinEvent>>>,
    _sub: gpui::Subscription,
}

fn rig() -> Rig {
    let mut s = Surface::open(
        PICTURE.width as f32 + 2. * PAD,
        PICTURE.height as f32 + 2. * PAD,
        |_| PinView::new(picture(), PICTURE, START),
    );
    let (events, sub) = s.record::<PinEvent>();
    Rig {
        s,
        events,
        _sub: sub,
    }
}

impl Rig {
    fn read<R>(&mut self, f: impl FnOnce(&PinView) -> R) -> R {
        let view = self.s.view.clone();
        self.s.app.cx.update(|cx| f(view.read(cx)))
    }

    /// The compositor moves the surface with the pin, so a pointer still on the output lands at a new surface-local point.
    fn local(&mut self, global: Point) -> (f32, f32) {
        let origin = self.read(PinView::origin);
        (
            (global.x - origin.x) as f32 + PAD,
            (global.y - origin.y) as f32 + PAD,
        )
    }

    fn scroll(&mut self, delta: ScrollDelta, control: bool) {
        self.s.send(PlatformInput::Scroll(ScrollEvent {
            position: point(px(PAD + 20.), px(PAD + 20.)),
            delta,
            modifiers: Modifiers {
                control,
                ..Modifiers::default()
            },
            ..ScrollEvent::default()
        }));
    }

    fn lines(&mut self, lines: f32, control: bool) {
        self.scroll(ScrollDelta::Lines(point(0., lines)), control);
    }

    fn right_click(&mut self, x: f32, y: f32) {
        self.s.hover(x, y, false);
        self.s.send(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Right,
            position: point(px(x), px(y)),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
    }

    fn resizes(&self) -> Vec<Size> {
        self.events
            .borrow()
            .iter()
            .filter_map(|e| match e {
                PinEvent::Resized(size) => Some(*size),
                _ => None,
            })
            .collect()
    }
}

#[test]
fn dragging_anywhere_on_the_picture_moves_it_with_the_pointer() {
    let mut r = rig();
    let press = START + (50., 40.);
    let (x, y) = r.local(press);
    r.s.hover(x, y, false);
    r.s.down(x, y);
    for step in 1..=3 {
        let (x, y) = r.local(press + (10. * f64::from(step), 5. * f64::from(step)));
        r.s.hover(x, y, true);
    }
    let (x, y) = r.local(press + (30., 15.));
    r.s.up(x, y);
    let moved = START + (30., 15.);
    assert_eq!(r.read(PinView::origin), moved);
    assert_eq!(r.events.borrow().last(), Some(&PinEvent::Moved(moved)));

    let (x, y) = r.local(press + (80., 80.));
    r.s.hover(x, y, false);
    assert_eq!(
        r.read(PinView::origin),
        moved,
        "once released, the pointer no longer carries it"
    );
}

#[test]
fn scrolling_resizes_from_the_top_left_keeping_its_shape_within_the_limits() {
    let mut r = rig();
    r.lines(1., false);
    assert!(same(r.read(PinView::size), scaled(PICTURE, 1.05)));
    r.scroll(ScrollDelta::Pixels(point(px(0.), px(40.))), false);
    assert!(
        same(r.read(PinView::size), scaled(PICTURE, 1.15)),
        "40 pixels scroll as two lines"
    );
    for _ in 0..100 {
        r.lines(1., false);
    }
    assert!(same(r.read(PinView::size), scaled(PICTURE, MAX_SCALE)));
    for _ in 0..200 {
        r.lines(-1., false);
    }
    assert!(same(r.read(PinView::size), scaled(PICTURE, MIN_SCALE)));
    assert_eq!(r.read(PinView::origin), START, "the top-left never moves");
    let resizes = r.resizes();
    assert!(
        resizes
            .iter()
            .all(|s| (s.width / s.height - PICTURE.width / PICTURE.height).abs() < 1e-9),
        "the picture keeps its aspect"
    );
    let at_limits = resizes.windows(2).filter(|w| w[0] == w[1]).count();
    assert_eq!(at_limits, 0, "scrolling past a limit resizes nothing");
}

#[test]
fn control_scroll_fades_the_picture_without_resizing_it() {
    let mut r = rig();
    let centre = (PAD + 150., PAD + 100.);
    let opaque = rgb(&r.s.app.screenshot("pin-opaque"), centre.0, centre.1);
    r.lines(-1., true);
    assert!((r.read(PinView::opacity) - 0.95).abs() < 1e-6);
    for _ in 0..40 {
        r.lines(-1., true);
    }
    assert_eq!(r.read(PinView::opacity), MIN_OPACITY);
    let faded = rgb(&r.s.app.screenshot("pin-faded"), centre.0, centre.1);
    assert!(
        !near(opaque, faded, 40),
        "the picture is drawn fainter: {opaque:?} -> {faded:?}"
    );
    for _ in 0..40 {
        r.lines(1., true);
    }
    assert_eq!(r.read(PinView::opacity), 1.);
    assert_eq!(r.read(PinView::size), PICTURE);
    assert!(r.resizes().is_empty());
}

#[test]
fn the_menu_copies_the_image_to_the_clipboard_and_closes_the_pin() {
    let mut r = rig();
    let copied: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    let sink = copied.clone();
    let clipboard: CopyImage = Rc::new(move |file: &Path, _| sink.borrow_mut().push(file.into()));
    let file = PathBuf::from("/captures/pinned.png");
    let closed = Rc::new(RefCell::new(false));
    let shut = closed.clone();
    let view = r.s.view.clone();
    let _route = r.s.app.cx.update(|cx| {
        cx.subscribe(&view, move |_, e: &PinEvent, cx| match e {
            PinEvent::CopyImage => clipboard(&file, cx),
            PinEvent::Close => *shut.borrow_mut() = true,
            _ => {}
        })
    });

    let (x, y) = (PAD + 40., PAD + 30.);
    r.right_click(x, y);
    assert!(r.read(PinView::menu_open));
    r.s.click(x + 30., y + 14.);
    assert_eq!(*copied.borrow(), [PathBuf::from("/captures/pinned.png")]);
    assert!(
        !r.read(PinView::menu_open),
        "choosing an item closes the menu"
    );
    assert!(!*closed.borrow());

    r.right_click(x, y);
    r.s.click(x + 30., y + 36.);
    assert!(*closed.borrow());
    assert_eq!(copied.borrow().len(), 1);
}

#[test]
fn the_hover_close_button_closes_the_pin() {
    let mut r = rig();
    let (x, y) = (PAD + PICTURE.width as f32 - 13., PAD + 13.);
    r.s.hover(x - 40., y + 40., false);
    r.s.click(x, y);
    assert_eq!(r.events.borrow().last(), Some(&PinEvent::Close));
    assert_eq!(
        r.read(PinView::origin),
        START,
        "pressing the button drags nothing"
    );
}
