// Each test binary uses its own subset of these helpers.
#![allow(dead_code)]

use es_testkit::Headless;
use es_ui::{assets::Assets, components::gallery};
use gpui::{
    AppContext, Context, Entity, Focusable, KeyDownEvent, Keystroke, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, OperatingSystem, PlatformInput,
    Render, Window, point, px,
};
use gpui_component::Root;

pub const WIDTH: f32 = 300.;
pub const GAP: f32 = 7.;

pub type App = Headless<Root>;

pub fn open<V: Render>(
    height: f32,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V,
) -> (App, Entity<V>) {
    let mut host = None;
    let mut app = Headless::open(Assets, gallery::init, WIDTH, height, |window, cx| {
        let view = cx.new(|cx| build(window, cx));
        host = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    });
    activate(&mut app);
    (app, host.expect("the window built its root"))
}

/// Focus events only fire in the active window, and a headless window opens inactive.
pub fn activate(app: &mut App) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.activate_window())
        .expect("window is open");
    app.settle();
}

pub fn send(app: &mut App, event: PlatformInput) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
        .expect("window is open");
    app.settle();
}

pub fn press(app: &mut App, x: f32, y: f32) {
    send(
        app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }),
    );
}

pub fn drag(app: &mut App, x: f32, y: f32) {
    send(
        app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: Some(MouseButton::Left),
            modifiers: Default::default(),
        }),
    );
}

pub fn release(app: &mut App, x: f32, y: f32) {
    send(
        app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Default::default(),
            click_count: 1,
        }),
    );
}

pub fn click(app: &mut App, x: f32, y: f32) {
    press(app, x, y);
    release(app, x, y);
}

pub fn stroke(keystroke: &str) -> Keystroke {
    Keystroke::parse_with_os(keystroke, OperatingSystem::Linux)
        .unwrap_or_else(|_| panic!("{keystroke} is not a keystroke"))
}

pub fn key(app: &mut App, keystroke: &str) {
    let keystroke = stroke(keystroke);
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .expect("window is open");
    app.settle();
}

pub fn held(app: &mut App, keystroke: &str) {
    send(
        app,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: stroke(keystroke),
            is_held: true,
            prefer_character_input: false,
        }),
    );
}

pub fn modifiers(app: &mut App, keystroke: &str) {
    send(
        app,
        PlatformInput::ModifiersChanged(ModifiersChangedEvent {
            modifiers: stroke(keystroke).modifiers,
            capslock: Default::default(),
        }),
    );
}

pub fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        key(app, &c.to_string());
    }
}

pub fn focus<V: Focusable>(app: &mut App, entity: &Entity<V>) {
    let entity = entity.clone();
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            let handle = entity.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        })
        .expect("window is open");
    app.settle();
}

pub fn blur(app: &mut App) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| window.blur(cx))
        .expect("window is open");
    app.settle();
}
