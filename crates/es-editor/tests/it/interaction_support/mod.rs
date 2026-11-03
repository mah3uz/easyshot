#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{App, canvas, send};
use es_editor::EditorSession;
use es_geom::Point;
use es_model::{AnnoShapeId, Tool};
use gpui::{
    CursorStyle, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, point, px,
};

/// The window position of a page point, through the live viewport and the canvas bounds.
pub fn at_page(app: &App, session: &EditorSession, x: f64, y: f64) -> gpui::Point<Pixels> {
    let vp = session.viewport.read_with(&app.cx, |v, _| v.viewport());
    let origin = canvas(app).read_with(&app.cx, |c, _| c.bounds().origin);
    let s = vp.page_to_screen(Point::new(x, y));
    origin + point(px(s.x as f32), px(s.y as f32))
}

pub fn down_with(app: &mut App, at: gpui::Point<Pixels>, modifiers: Modifiers) {
    send(
        app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers,
            click_count: 1,
            ..MouseDownEvent::default()
        }),
    );
}

pub fn down(app: &mut App, at: gpui::Point<Pixels>) {
    down_with(app, at, Modifiers::default());
}

pub fn drag_to(app: &mut App, at: gpui::Point<Pixels>) {
    send(
        app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: at,
            pressed_button: Some(MouseButton::Left),
            ..MouseMoveEvent::default()
        }),
    );
}

pub fn hover(app: &mut App, at: gpui::Point<Pixels>) {
    send(
        app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: at,
            ..MouseMoveEvent::default()
        }),
    );
}

pub fn up(app: &mut App, at: gpui::Point<Pixels>) {
    send(
        app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            click_count: 1,
            ..MouseUpEvent::default()
        }),
    );
}

/// A press, `steps` moves along the straight line and a release, all in window pixels.
pub fn drag(app: &mut App, from: gpui::Point<Pixels>, to: gpui::Point<Pixels>, steps: usize) {
    down(app, from);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        drag_to(app, from + (to - from) * t);
    }
    up(app, to);
}

/// Picking the armed tool again would toggle back to Select, so an armed tool is left alone.
pub fn set_tool(app: &mut App, session: &EditorSession, tool: Tool) {
    if session.doc.read_with(&app.cx, |d, _| d.editor().tool()) == tool {
        return;
    }
    app.cx.update(|cx| {
        session
            .doc
            .update(cx, |d, cx| d.apply(cx, |e| e.select_tool(tool)))
    });
    app.settle();
}

pub fn select(app: &mut App, session: &EditorSession, ids: Vec<AnnoShapeId>) {
    app.cx.update(|cx| {
        session
            .doc
            .update(cx, |d, cx| d.apply(cx, |e| e.select(ids)))
    });
    app.settle();
}

pub fn cursor(app: &App) -> CursorStyle {
    canvas(app).read_with(&app.cx, |c, _| c.cursor())
}

pub fn ids(app: &App, session: &EditorSession) -> Vec<AnnoShapeId> {
    session.doc.read_with(&app.cx, |d, _| {
        d.editor().doc().shapes().iter().map(|s| s.id).collect()
    })
}

pub fn shape(app: &App, session: &EditorSession, id: AnnoShapeId) -> es_model::AnnoShape {
    session.doc.read_with(&app.cx, |d, _| {
        d.editor()
            .doc()
            .shape(id)
            .cloned()
            .expect("the shape exists")
    })
}

pub fn page_bounds(app: &App, session: &EditorSession, id: AnnoShapeId) -> es_geom::BBox {
    session.doc.read_with(&app.cx, |d, _| {
        d.editor()
            .doc()
            .page_bounds(id)
            .expect("the shape has bounds")
    })
}

pub fn selected(app: &App, session: &EditorSession) -> Vec<AnnoShapeId> {
    session.doc.read_with(&app.cx, |d, _| {
        d.editor().selected_ids().iter().copied().collect()
    })
}

/// A page whose viewport is zoomed and panned, so a page/screen mix-up lands visibly off.
pub fn zoomed(app: &mut App, session: &EditorSession) {
    app.cx.update(|cx| {
        session.viewport.update(cx, |v, cx| {
            v.set_percent(130.0, cx);
            v.pan_by(-60.0, 25.0, cx);
        })
    });
    app.settle();
}
