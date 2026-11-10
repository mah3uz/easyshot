use es_app::platform_ui::ToastHost;
use es_app::toast::{
    Countdown, ERROR_DURATION, IconName, TOAST_DURATION, Toast, ToastRequest, ToastStyle,
    UNDO_DURATION,
};
use es_ui::motion::TOAST_OUT;
use gpui::{Modifiers, MouseExitEvent, MouseMoveEvent, PlatformInput, point, px};
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
        let request = ToastRequest {
            title: "Could not save".into(),
            message: None,
            icon: IconName::TriangleAlert,
            style: ToastStyle::Error,
            duration: ERROR_DURATION,
            output: None,
            undo: None,
        };
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

    fn leaving(&mut self) -> bool {
        let host = self.host.clone();
        self.app.cx.update(|cx| host.read(cx).leaving())
    }
}

#[test]
fn an_error_toast_waits_while_hovered_and_closes_once_the_pointer_has_left_its_surface() {
    let mut t = Shown::error();
    let on_card = point(px(210.), px(40.));
    let enter = |t: &mut Shown| {
        t.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: on_card,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }))
    };
    // As on Wayland, where the input region is the card: a leave with no move off it, then a redraw.
    let leave = |t: &mut Shown| {
        t.send(PlatformInput::MouseExited(MouseExitEvent {
            position: on_card,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }));
        t.redraw();
    };
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
