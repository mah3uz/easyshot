use crate::support;

use std::{cell::RefCell, rc::Rc};

use gpui::{
    Bounds, Context, InteractiveElement, IntoElement, Modifiers, MouseExitEvent, MouseMoveEvent,
    ParentElement, Pixels, PlatformInput, Render, StatefulInteractiveElement, Styled, Window,
    canvas, div, point, px,
};
use support::*;

const SIDE: f32 = 40.;
const HOVER_HEIGHT: f32 = 60.;
const GROUP_WIDTH: f32 = 10.;
const GROUP_HOVER_WIDTH: f32 = 30.;

#[derive(Default)]
struct Seen {
    hover_events: Vec<bool>,
    exits: usize,
    control: Option<Bounds<Pixels>>,
    group_child: Option<Bounds<Pixels>>,
}

struct EdgeControl {
    seen: Rc<RefCell<Seen>>,
}

impl Render for EdgeControl {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (events, exits, control, group_child) = (
            self.seen.clone(),
            self.seen.clone(),
            self.seen.clone(),
            self.seen.clone(),
        );
        div().size_full().child(
            div()
                .id("edge")
                .group("edge")
                .absolute()
                .left_0()
                .top_0()
                .size(px(SIDE))
                .hover(|s| s.h(px(HOVER_HEIGHT)))
                .on_hover(move |over, _, _| events.borrow_mut().hover_events.push(*over))
                .on_mouse_exit(move |_, _, _| exits.borrow_mut().exits += 1)
                .child(
                    canvas(
                        move |b, _, _| control.borrow_mut().control = Some(b),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .child(
                    div()
                        .id("edge-group-child")
                        .h(px(5.))
                        .w(px(GROUP_WIDTH))
                        .group_hover("edge", |s| s.w(px(GROUP_HOVER_WIDTH)))
                        .child(
                            canvas(
                                move |b, _, _| group_child.borrow_mut().group_child = Some(b),
                                |_, _, _, _| {},
                            )
                            .size_full(),
                        ),
                ),
        )
    }
}

fn redraw(app: &mut App) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .expect("window is open");
    app.settle();
}

fn looks(seen: &Rc<RefCell<Seen>>) -> (f32, f32) {
    let s = seen.borrow();
    let height = s.control.expect("control painted").size.height;
    let width = s.group_child.expect("group child painted").size.width;
    (f32::from(height), f32::from(width))
}

#[test]
fn a_control_at_the_window_edge_stops_being_hovered_when_the_pointer_leaves_the_window() {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let shared = seen.clone();
    let (mut app, _) = open(100., move |_, _| EdgeControl { seen: shared });
    let at_edge = point(px(1.), px(20.));
    let enter = |app: &mut App| {
        send(
            app,
            PlatformInput::MouseMove(MouseMoveEvent {
                position: at_edge,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
        );
        redraw(app);
    };

    enter(&mut app);
    assert_eq!(seen.borrow().hover_events, [true]);
    assert_eq!(
        looks(&seen),
        (HOVER_HEIGHT, GROUP_HOVER_WIDTH),
        "hover styles on"
    );

    // As on Wayland: the pointer leaves with no move past the edge, then the window redraws.
    send(
        &mut app,
        PlatformInput::MouseExited(MouseExitEvent {
            position: at_edge,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
    );
    redraw(&mut app);
    redraw(&mut app);
    assert_eq!(
        seen.borrow().hover_events,
        [true, false],
        "on_hover(false) fires once and hover does not come back on the redraw"
    );
    assert_eq!(looks(&seen), (SIDE, GROUP_WIDTH), "hover styles off");
    assert_eq!(
        seen.borrow().exits,
        1,
        "the control hears the pointer leave"
    );

    enter(&mut app);
    assert_eq!(seen.borrow().hover_events, [true, false, true]);
    assert_eq!(
        looks(&seen),
        (HOVER_HEIGHT, GROUP_HOVER_WIDTH),
        "hovered again once the pointer is back"
    );
}
