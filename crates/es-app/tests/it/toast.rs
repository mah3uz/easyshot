use es_app::platform_ui::ToastHost;
use es_app::toast::{
    ERROR_DURATION, IconName, TOAST_DURATION, Toast, ToastRequest, ToastStyle, ToastUndo,
    UNDO_DURATION,
};
use es_ui::components::Countdown;
use es_ui::motion::TOAST_OUT;
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, point, px,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const SECOND: Duration = Duration::from_secs(1);

#[test]
fn an_error_lasts_longer_than_a_notice_and_less_than_an_undo() {
    assert_eq!(
        Toast::error("toast-open-unsupported").duration,
        ERROR_DURATION
    );
    assert_eq!(ERROR_DURATION, Duration::from_secs(5));
    assert_eq!(TOAST_DURATION, Duration::from_millis(2500));
    assert!(ERROR_DURATION < UNDO_DURATION);
}

#[test]
fn hovering_holds_the_time_and_leaving_resumes_with_what_was_left() {
    let mut countdown = Countdown::new(ERROR_DURATION);
    assert!(!countdown.advance(2 * SECOND, false));
    assert!(
        !countdown.advance(60 * SECOND, true),
        "still shown while hovered"
    );
    assert_eq!(countdown.remaining(), 3 * SECOND);
    assert!(!countdown.advance(2 * SECOND, false));
    assert!(
        countdown.advance(SECOND, false),
        "closes after the remaining time"
    );
}

struct Shown {
    app: es_testkit::Headless<ToastHost>,
    host: gpui::Entity<ToastHost>,
    closed: Rc<Cell<bool>>,
}

impl Shown {
    fn error() -> Self {
        Self::open(ToastRequest {
            title: "Could not save".into(),
            message: None,
            icon: IconName::TriangleAlert,
            style: ToastStyle::Error,
            duration: ERROR_DURATION,
            output: None,
            undo: None,
            actions: Vec::new(),
            progress: None,
            sticky: false,
            key: None,
            on_close: None,
            closed: Default::default(),
        })
    }

    fn undo(undone: Rc<Cell<bool>>) -> Self {
        Self::open(ToastRequest {
            title: "Capture deleted".into(),
            message: None,
            icon: IconName::Delete,
            style: ToastStyle::Normal,
            duration: UNDO_DURATION,
            output: None,
            undo: Some(("Undo".into(), ToastUndo::new(move |_| undone.set(true)))),
            actions: Vec::new(),
            progress: None,
            sticky: false,
            key: None,
            on_close: None,
            closed: Default::default(),
        })
    }

    fn open(request: ToastRequest) -> Self {
        let closed = Rc::new(Cell::new(false));
        let done = closed.clone();
        let mut host = None;
        let app =
            es_testkit::Headless::open(es_ui::assets::Assets, es_ui::init, 420., 132., |_, cx| {
                let h = ToastHost::start(request, move |_| done.set(true), cx);
                host = Some(h.clone());
                h
            });
        let host = host.expect("the window built the toast");
        Self { app, host, closed }
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

    fn redraw(&mut self) {
        let window = self.app.window.into();
        self.app
            .cx
            .update_window(window, |_, window, _| window.refresh())
            .expect("window is open");
        self.app.settle();
    }

    fn wait(&mut self, time: Duration) {
        self.app.cx.advance_clock(time);
        self.app.settle();
    }

    fn pointer(&mut self, position: Point<Pixels>) {
        self.send(PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
        self.redraw();
    }

    // As on Wayland, where the input region is the card: a leave with no move off it, then a redraw.
    fn leave(&mut self, last: Point<Pixels>) {
        self.send(PlatformInput::MouseExited(MouseExitEvent {
            position: last,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
        self.redraw();
    }

    fn press(&mut self, position: Point<Pixels>, pressed: bool) {
        self.send(if pressed {
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            })
        } else {
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            })
        });
    }

    fn leaving(&mut self) -> bool {
        let host = self.host.clone();
        self.app.cx.update(|cx| host.read(cx).leaving())
    }
}

#[test]
fn an_error_toast_waits_while_hovered_and_closes_once_the_pointer_has_left_its_surface() {
    let mut t = Shown::error();
    let on_card = point(px(210.), px(40.));
    let enter = |t: &mut Shown| t.pointer(on_card);
    let leave = |t: &mut Shown| t.leave(on_card);
    t.wait(2 * SECOND);
    enter(&mut t);
    t.wait(ERROR_DURATION * 4);
    assert!(!t.leaving(), "held while hovered");
    leave(&mut t);
    t.wait(SECOND);
    enter(&mut t);
    t.wait(ERROR_DURATION * 4);
    assert!(!t.leaving(), "held again when the pointer comes back");
    leave(&mut t);
    t.wait(SECOND);
    assert!(!t.leaving(), "the time left when hovered still runs");
    t.wait(2 * SECOND);
    assert!(t.leaving(), "goes once the time left has run out");
    t.wait(TOAST_OUT.duration + SECOND);
    assert!(t.closed.get(), "closes after its exit");
}

const ON_UNDO: Point<Pixels> = point(px(270.), px(38.));
const ON_CARD: Point<Pixels> = point(px(150.), px(38.));
const OFF_CARD: Point<Pixels> = point(px(400.), px(110.));

fn held_on_undo_then_gone(t: &mut Shown, leave: impl FnOnce(&mut Shown)) {
    t.wait(SECOND);
    t.pointer(ON_CARD);
    t.pointer(ON_UNDO);
    t.wait(UNDO_DURATION * 3);
    assert!(!t.leaving(), "held while the pointer is on Undo");
    leave(t);
    t.wait(UNDO_DURATION - 2 * SECOND);
    assert!(!t.leaving(), "the time left when hovered still runs");
    t.wait(2 * SECOND);
    assert!(t.leaving(), "goes once the time left has run out");
    t.wait(TOAST_OUT.duration + SECOND);
    assert!(t.closed.get(), "closes after its exit");
}

#[test]
fn an_undo_toast_held_on_its_button_closes_once_the_pointer_has_left_its_surface() {
    let undone = Rc::new(Cell::new(false));
    let mut t = Shown::undo(undone.clone());
    held_on_undo_then_gone(&mut t, |t| t.leave(ON_UNDO));
    assert!(!undone.get(), "hovering never presses Undo");
}

#[test]
fn an_undo_toast_held_on_its_button_closes_once_the_pointer_has_moved_off_the_card() {
    let mut t = Shown::undo(Rc::new(Cell::new(false)));
    held_on_undo_then_gone(&mut t, |t| {
        t.pointer(OFF_CARD);
        t.leave(OFF_CARD);
    });
}

#[test]
fn an_undo_toast_closes_after_undo_is_pressed_and_released_off_it() {
    let undone = Rc::new(Cell::new(false));
    let mut t = Shown::undo(undone.clone());
    held_on_undo_then_gone(&mut t, |t| {
        t.press(ON_UNDO, true);
        t.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: OFF_CARD,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }));
        t.press(OFF_CARD, false);
        t.leave(OFF_CARD);
    });
    assert!(!undone.get(), "a press released off Undo is not a click");
}

#[test]
fn pressing_undo_undoes_and_closes_the_toast() {
    let undone = Rc::new(Cell::new(false));
    let mut t = Shown::undo(undone.clone());
    t.wait(SECOND);
    t.pointer(ON_UNDO);
    t.press(ON_UNDO, true);
    t.press(ON_UNDO, false);
    assert!(undone.get(), "the press lands on Undo");
    assert!(t.closed.get(), "the toast closes at once");
}
