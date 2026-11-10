use es_app::platform_ui::ComingSoonHost;
use es_ui::components::Feature;
use es_ui::motion::PANEL_OUT;
use gpui::{
    Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point, px,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const W: f32 = 460.;
const H: f32 = 300.;
/// The close button sits 10 px in from the top right of a 340 px panel centred in the window.
const ON_CLOSE: (f32, f32) = (376., 62.);
const ON_PANEL: (f32, f32) = (230., 200.);
const ON_SCRIM: (f32, f32) = (20., 20.);

struct Shown {
    app: es_testkit::Headless<ComingSoonHost>,
    host: gpui::Entity<ComingSoonHost>,
    closed: Rc<Cell<bool>>,
}

impl Shown {
    fn open(feature: Feature) -> Self {
        let closed = Rc::new(Cell::new(false));
        let done = closed.clone();
        let mut host = None;
        let app = es_testkit::Headless::open(es_ui::assets::Assets, es_ui::init, W, H, |_, cx| {
            let h = ComingSoonHost::new(feature, move |_| done.set(true), cx);
            host = Some(h.clone());
            h
        });
        let host = host.expect("the window built the panel");
        let mut shown = Self { app, host, closed };
        let window = shown.app.window.into();
        shown
            .app
            .cx
            .update_window(window, |_, window, _| window.activate_window())
            .expect("window is open");
        shown.app.settle();
        shown
    }

    fn send(&mut self, event: PlatformInput) {
        let window = self.app.window.into();
        self.app
            .cx
            .update_window(window, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .expect("window is open");
        self.app.settle();
    }

    fn click(&mut self, (x, y): (f32, f32)) {
        let position = point(px(x), px(y));
        self.send(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.send(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }));
    }

    fn key(&mut self, key: &'static str) {
        let keystroke = Keystroke::parse(key).expect("a keystroke");
        let window = self.app.window.into();
        self.app
            .cx
            .update_window(window, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .expect("window is open");
        self.app.settle();
    }

    fn leaving(&mut self) -> bool {
        let host = self.host.clone();
        self.app.cx.update(|cx| host.read(cx).leaving())
    }

    /// Leaving at once, then closed only once the exit has played.
    fn closes_after_its_exit(&mut self) {
        assert!(self.leaving(), "the exit starts at once");
        assert!(!self.closed.get(), "the surface stays while the exit plays");
        self.app
            .cx
            .advance_clock(PANEL_OUT.duration + Duration::from_millis(20));
        self.app.settle();
        assert!(self.closed.get(), "closed once the exit has played");
    }
}

#[test]
fn the_close_button_closes_the_panel() {
    let mut s = Shown::open(Feature::CloudShare);
    s.click(ON_CLOSE);
    s.closes_after_its_exit();
}

#[test]
fn esc_closes_the_panel_without_a_click() {
    let mut s = Shown::open(Feature::VideoStudio);
    s.key("escape");
    s.closes_after_its_exit();
}

#[test]
fn a_press_outside_the_panel_closes_it_and_one_on_it_does_not() {
    let mut s = Shown::open(Feature::VideoStudio);
    s.click(ON_PANEL);
    assert!(!s.leaving(), "reading the panel is not a dismissal");
    s.key("a");
    assert!(!s.leaving(), "only Esc closes from the keyboard");
    s.click(ON_SCRIM);
    s.closes_after_its_exit();
}
