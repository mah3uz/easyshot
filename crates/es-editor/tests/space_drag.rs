#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod interaction_support;

use canvas_support::*;
use es_geom::Point;
use es_model::{ShapeKind, Tool};
use gpui::{
    KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton, MouseMoveEvent, MouseUpEvent,
    PlatformInput, point, px,
};
use interaction_support::*;

fn setup() -> (App, es_editor::EditorSession) {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![rect(550.0, 290.0, 100.0, 60.0)],
            Default::default(),
        ),
    );
    app.cx.update(|cx| {
        session.viewport.update(cx, |v, cx| {
            v.set_percent(400.0, cx);
            v.pan_by(-1e6, -1e6, cx);
            let max = v.math().max_pan();
            v.pan_by(max.width, max.height, cx);
        })
    });
    app.settle();
    let handle = canvas(&app).read_with(&app.cx, |c, _| c.focus_handle().clone());
    let window = app.window.into();
    app.cx
        .update_window(window, |_, w, cx| w.focus(&handle, cx))
        .unwrap();
    app.settle();
    (app, session)
}

fn space(app: &mut App, down: bool) {
    let keystroke = Keystroke::parse("space").unwrap();
    send(
        app,
        if down {
            PlatformInput::KeyDown(KeyDownEvent {
                keystroke,
                is_held: false,
                prefer_character_input: false,
            })
        } else {
            PlatformInput::KeyUp(KeyUpEvent { keystroke })
        },
    );
}

fn moved(app: &mut App, to: gpui::Point<gpui::Pixels>, pressed: Option<MouseButton>) {
    send(
        app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: to,
            pressed_button: pressed,
            modifiers: Modifiers::default(),
        }),
    );
}

fn origin(app: &App, session: &es_editor::EditorSession) -> Point {
    session.viewport.read_with(&app.cx, |v, _| {
        v.viewport().page_to_screen(Point::new(0.0, 0.0))
    })
}

fn shape_pos(app: &App, session: &es_editor::EditorSession) -> (f64, f64) {
    let s = shape(app, session, ids(app, session)[0]);
    (s.x, s.y)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1.0
}

#[test]
fn space_drag_on_a_shape_moves_it_in_one_undo_step() {
    let (mut app, session) = setup();
    set_tool(&mut app, &session, Tool::Select);
    let (x0, y0) = shape_pos(&app, &session);
    let view0 = origin(&app, &session);
    let (from, to) = (
        at_page(&app, &session, 550.0, 320.0),
        at_page(&app, &session, 570.0, 330.0),
    );
    space(&mut app, true);
    hover(&mut app, from);
    assert_eq!(cursor(&app), gpui::CursorStyle::PointingHand);
    drag(&mut app, from, to, 8);
    space(&mut app, false);

    let (x1, y1) = shape_pos(&app, &session);
    assert!(
        near(x1 - x0, 20.0) && near(y1 - y0, 10.0),
        "{x0},{y0} -> {x1},{y1}"
    );
    assert_eq!(origin(&app, &session), view0, "the view did not pan");
    app.cx.update(|cx| session.undo(cx));
    assert_eq!(shape_pos(&app, &session), (x0, y0), "one undo restores it");
}

#[test]
fn space_drag_on_empty_canvas_pans_and_leaves_shapes_alone() {
    let (mut app, session) = setup();
    set_tool(&mut app, &session, Tool::Select);
    let shape0 = shape_pos(&app, &session);
    let view0 = origin(&app, &session);
    let (from, to) = (
        at_page(&app, &session, 680.0, 380.0),
        at_page(&app, &session, 690.0, 385.0),
    );
    space(&mut app, true);
    hover(&mut app, from);
    assert_eq!(cursor(&app), gpui::CursorStyle::OpenHand);
    drag(&mut app, from, to, 8);
    space(&mut app, false);

    assert_ne!(origin(&app, &session), view0, "the view panned");
    assert_eq!(shape_pos(&app, &session), shape0);
}

#[test]
fn space_drag_on_a_shape_with_a_drawing_tool_moves_it_and_draws_nothing() {
    let (mut app, session) = setup();
    set_tool(&mut app, &session, Tool::Rectangle);
    let (x0, y0) = shape_pos(&app, &session);
    let (from, to) = (
        at_page(&app, &session, 550.0, 320.0),
        at_page(&app, &session, 565.0, 335.0),
    );
    space(&mut app, true);
    drag(&mut app, from, to, 6);
    space(&mut app, false);

    assert_eq!(ids(&app, &session).len(), 1, "no shape was created");
    let (x1, y1) = shape_pos(&app, &session);
    assert!(near(x1 - x0, 15.0) && near(y1 - y0, 15.0));
}

#[test]
fn releasing_space_ends_the_pan_even_while_the_pointer_keeps_moving() {
    let (mut app, session) = setup();
    let a = at_page(&app, &session, 680.0, 380.0);
    space(&mut app, true);
    down(&mut app, a);
    drag_to(&mut app, a + point(px(-20.), px(-10.)));
    space(&mut app, false);
    let view = origin(&app, &session);
    moved(&mut app, a + point(px(-60.), px(-40.)), None);
    assert_eq!(origin(&app, &session), view);
}

#[test]
fn a_release_outside_the_canvas_ends_the_pan() {
    let (mut app, session) = setup();
    let a = at_page(&app, &session, 680.0, 380.0);
    space(&mut app, true);
    down(&mut app, a);
    drag_to(&mut app, a + point(px(-20.), px(-10.)));
    send(
        &mut app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(-30.), px(-30.)),
            click_count: 1,
            modifiers: Modifiers::default(),
        }),
    );
    let view = origin(&app, &session);
    moved(&mut app, a + point(px(-60.), px(-40.)), None);
    assert_eq!(origin(&app, &session), view);
}

#[test]
fn a_move_with_no_button_held_ends_a_pan_whose_release_was_never_seen() {
    let (mut app, session) = setup();
    let a = at_page(&app, &session, 680.0, 380.0);
    space(&mut app, true);
    down(&mut app, a);
    drag_to(&mut app, a + point(px(-20.), px(-10.)));
    let view = origin(&app, &session);
    moved(&mut app, a + point(px(-60.), px(-40.)), None);
    moved(&mut app, a + point(px(-90.), px(-70.)), None);
    assert_eq!(
        origin(&app, &session),
        view,
        "the pointer alone does not pan"
    );
}

#[test]
fn a_new_text_box_starts_at_size_thirty_six() {
    let (mut app, session) = setup();
    set_tool(&mut app, &session, Tool::Text);
    let at = at_page(&app, &session, 700.0, 450.0);
    down(&mut app, at);
    up(&mut app, at);
    let made = ids(&app, &session);
    let text = shape(&app, &session, *made.last().unwrap());
    match text.kind {
        ShapeKind::Text(t) => assert_eq!(t.font_size, 36.0),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_cursor_while_space_is_held_is_open_hand_closed_hand_when_panning_and_pointing_over_a_shape()
{
    let (mut app, session) = setup();
    let empty = at_page(&app, &session, 680.0, 380.0);
    let on_shape = at_page(&app, &session, 550.0, 320.0);
    space(&mut app, true);
    hover(&mut app, empty);
    assert_eq!(cursor(&app), gpui::CursorStyle::OpenHand);

    down(&mut app, empty);
    drag_to(&mut app, empty + point(px(5.), px(5.)));
    assert_eq!(cursor(&app), gpui::CursorStyle::ClosedHand, "panning");
    up(&mut app, empty);

    hover(&mut app, on_shape);
    assert_eq!(cursor(&app), gpui::CursorStyle::PointingHand);
    down(&mut app, on_shape);
    drag_to(&mut app, on_shape + point(px(30.), px(30.)));
    assert_eq!(
        cursor(&app),
        gpui::CursorStyle::PointingHand,
        "still the move cursor while dragging the shape"
    );
    up(&mut app, on_shape);
}
