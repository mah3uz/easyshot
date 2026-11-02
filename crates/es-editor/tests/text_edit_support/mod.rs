#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{App, canvas, send, swatch};
use es_editor::EditorSession;
use es_editor::canvas::TextEditState;
use es_model::{AnnoShape, AnnoShapeId, ShapeKind, TextAlign, TextProps};
use gpui::{
    Bounds, EntityInputHandler, KeyDownEvent, Keystroke, Pixels, PlatformInput, UTF16Selection,
};
use std::ops::Range;

pub fn text_shape(x: f64, y: f64, text: &str, align: TextAlign, rotation: f64) -> AnnoShape {
    let mut shape = AnnoShape::new(
        x,
        y,
        ShapeKind::Text(TextProps {
            text: text.into(),
            font_size: 48.0,
            swatch: swatch(0.85, 0.15, 0.2),
            align,
            ..TextProps::default()
        }),
    );
    shape.rotation = rotation;
    shape
}

pub fn text_of(app: &App, session: &EditorSession, id: AnnoShapeId) -> Option<String> {
    session
        .doc
        .read_with(&app.cx, |d, _| match &d.editor().doc().shape(id)?.kind {
            ShapeKind::Text(p) => Some(p.text.clone()),
            _ => None,
        })
}

pub fn editing(app: &App, session: &EditorSession) -> Option<AnnoShapeId> {
    session
        .doc
        .read_with(&app.cx, |d, _| d.editor().editing_text_id())
}

pub fn begin(app: &mut App, session: &EditorSession, id: AnnoShapeId) {
    app.cx.update(|cx| {
        session
            .doc
            .update(cx, |d, cx| d.apply(cx, |e| e.begin_text(id)))
    });
    input(app, |c, window, cx| {
        window.focus(&c.focus_handle().clone(), cx)
    });
}

pub fn state(app: &App) -> TextEditState {
    canvas(app)
        .read_with(&app.cx, |c, _| c.text_edit_state())
        .expect("a text is being edited")
}

/// Runs `f` against the canvas's input handler, the way the platform's text input reaches it.
pub fn input<R>(
    app: &mut App,
    f: impl FnOnce(
        &mut es_editor::EditorCanvas,
        &mut gpui::Window,
        &mut gpui::Context<es_editor::EditorCanvas>,
    ) -> R,
) -> R {
    let canvas = canvas(app);
    let handle = app.window.into();
    let out = app
        .cx
        .update_window(handle, |_, window, cx| {
            canvas.update(cx, |c, cx| f(c, window, cx))
        })
        .expect("window is open");
    app.settle();
    out
}

pub fn type_text(app: &mut App, text: &str) {
    input(app, |c, w, cx| c.replace_text_in_range(None, text, w, cx));
}

pub fn compose(app: &mut App, text: &str) {
    let n = text.encode_utf16().count();
    input(app, |c, w, cx| {
        c.replace_and_mark_text_in_range(None, text, Some(n..n), w, cx)
    });
}

pub fn marked(app: &mut App) -> Option<Range<usize>> {
    input(app, |c, w, cx| c.marked_text_range(w, cx))
}

pub fn selection_utf16(app: &mut App) -> Option<UTF16Selection> {
    input(app, |c, w, cx| c.selected_text_range(false, w, cx))
}

pub fn ime_bounds(app: &mut App, range: Range<usize>) -> Bounds<Pixels> {
    let bounds = canvas(app).read_with(&app.cx, |c, _| c.bounds());
    input(app, |c, w, cx| c.bounds_for_range(range, bounds, w, cx)).expect("bounds for the range")
}

pub fn key(app: &mut App, keystroke: &'static str) {
    send(
        app,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(keystroke).expect("a valid keystroke"),
            is_held: false,
            prefer_character_input: false,
        }),
    );
}
