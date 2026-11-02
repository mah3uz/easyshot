#![allow(dead_code)]

use es_geom::Rect;
use es_model::platform::{Frame, OutputId, Transform};
use es_testkit::Headless;
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, KeyDownEvent, Keystroke, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput, Render, Subscription,
    point, px,
};
use image::RgbaImage;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

// Headless windows render at scale 2.
pub const WINDOW_SCALE: f32 = 2.;

/// `scale` frame pixels per logical point; `colour` is asked per frame pixel.
pub fn frame(w: f32, h: f32, scale: f64, colour: impl Fn(u32, u32) -> [u8; 3]) -> Arc<Frame> {
    let (pw, ph) = (
        (f64::from(w) * scale).round() as u32,
        (f64::from(h) * scale).round() as u32,
    );
    let mut bgra = Vec::with_capacity((pw * ph * 4) as usize);
    for y in 0..ph {
        for x in 0..pw {
            let [r, g, b] = colour(x, y);
            bgra.extend_from_slice(&[b, g, r, 255]);
        }
    }
    Arc::new(Frame {
        output: Some(OutputId("HEADLESS-1".into())),
        size_px: (pw, ph),
        logical: Rect::new(0., 0., f64::from(w), f64::from(h)),
        scale,
        bgra: Arc::from(bgra),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    })
}

pub struct Surface<V: Render + 'static> {
    pub app: Headless<V>,
    pub view: Entity<V>,
}

impl<V: Render + 'static> Surface<V> {
    pub fn open(w: f32, h: f32, build: impl FnOnce(&mut Context<V>) -> V) -> Self {
        let mut view = None;
        let mut app = Headless::open(es_ui::assets::Assets, es_ui::hello::init, w, h, |_, cx| {
            let v = cx.new(build);
            view = Some(v.clone());
            v
        });
        let view = view.expect("the window built its root");
        let handle = app.window.into();
        app.cx
            .update_window(handle, |_, window, _| window.activate_window())
            .expect("window is open");
        app.settle();
        Surface { app, view }
    }

    pub fn record<E: Clone + 'static>(&mut self) -> (Rc<RefCell<Vec<E>>>, Subscription)
    where
        V: EventEmitter<E>,
    {
        let events = Rc::new(RefCell::new(Vec::new()));
        let seen = events.clone();
        let view = self.view.clone();
        let sub = self
            .app
            .cx
            .update(|cx| cx.subscribe(&view, move |_, e: &E, _| seen.borrow_mut().push(e.clone())));
        (events, sub)
    }

    pub fn send(&mut self, event: PlatformInput) {
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .expect("window is open");
        self.app.settle();
    }

    pub fn hover(&mut self, x: f32, y: f32, pressed: bool) {
        self.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: pressed.then_some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }));
    }

    pub fn down(&mut self, x: f32, y: f32) {
        self.send(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
    }

    pub fn up(&mut self, x: f32, y: f32) {
        self.send(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    pub fn click(&mut self, x: f32, y: f32) {
        self.hover(x, y, false);
        self.down(x, y);
        self.up(x, y);
    }

    pub fn key(&mut self, key: &'static str) {
        self.send(PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("keystroke"),
            is_held: false,
            prefer_character_input: false,
        }));
    }
}

/// The screenshot pixel at a logical point.
pub fn rgb(shot: &RgbaImage, x: f32, y: f32) -> [u8; 3] {
    let p = shot
        .get_pixel((x * WINDOW_SCALE) as u32, (y * WINDOW_SCALE) as u32)
        .0;
    [p[0], p[1], p[2]]
}

pub fn near(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    (0..3).all(|i| a[i].abs_diff(b[i]) <= tolerance)
}

/// Blue clearly outweighs red, as the accent does.
pub fn bluish(p: [u8; 3]) -> bool {
    i32::from(p[2]) > i32::from(p[0]) + 60
}
