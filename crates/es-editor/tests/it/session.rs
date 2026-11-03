#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::*;
use es_editor::session::viewport::{CANVAS_INSET, ViewMath};
use es_editor::{EditorSession, EditorSource, Error};
use es_geom::{Point, Size};
use es_model::BackgroundSettings;
use es_render::beautify::layout;
use std::cell::RefCell;
use std::rc::Rc;

fn math(image: (f64, f64), container: (f64, f64)) -> ViewMath {
    let mut m = ViewMath::new(layout(image.0, image.1, &BackgroundSettings::default()));
    m.container = Size::new(container.0, container.1);
    m.scale_factor = 2.0;
    m
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn fit_leaves_the_workspace_inset_and_centres_the_canvas() {
    let m = math((2000.0, 1000.0), (1068.0, 556.0));
    let s = m.fit_scale();
    assert!(close(s, 0.5), "fit {s}");
    let r = m.canvas_rect();
    assert!(close(r.x0, CANVAS_INSET.width) && close(r.y0, (556.0 - 500.0) / 2.0));
    assert_eq!(
        m.zoom_percent(),
        100.0,
        "0.5 logical px is one device px at 2x"
    );
}

#[test]
fn zoom_is_clamped_to_ten_and_four_hundred_percent() {
    let mut m = math((1200.0, 640.0), (800.0, 600.0));
    m.set_percent(5.0);
    assert_eq!(m.zoom_percent(), 10.0);
    m.set_percent(900.0);
    assert_eq!(m.zoom_percent(), 400.0);
    m.zoom_at(10.0, m.centre());
    assert_eq!(m.zoom_percent(), 400.0);
}

#[test]
fn steps_multiply_by_five_quarters_and_four_fifths() {
    let mut m = math((1200.0, 640.0), (800.0, 600.0));
    m.set_percent(100.0);
    m.zoom_at(1.25, m.centre());
    assert_eq!(m.zoom_percent(), 125.0);
    m.zoom_at(0.8, m.centre());
    m.zoom_at(0.8, m.centre());
    assert_eq!(m.zoom_percent(), 80.0);
}

#[test]
fn zooming_at_the_pointer_keeps_the_page_point_under_it() {
    let mut m = math((3000.0, 2000.0), (800.0, 600.0));
    m.set_percent(100.0);
    for (factor, at) in [
        (1.3, (120.0, 90.0)),
        (0.85, (700.0, 500.0)),
        (2.0, (400.0, 10.0)),
    ] {
        let pointer = Point::new(at.0, at.1);
        let before = m.viewport().screen_to_page(pointer);
        m.zoom_at(factor, pointer);
        let after = m.viewport().screen_to_page(pointer);
        assert!(
            close(before.x, after.x) && close(before.y, after.y),
            "x{factor} at {at:?}: {before:?} became {after:?}"
        );
    }
}

#[test]
fn pan_is_clamped_so_the_canvas_cannot_leave_the_view() {
    let mut m = math((1200.0, 640.0), (800.0, 600.0));
    m.pan_by(500.0, 500.0);
    assert_eq!(m.pan, Point::ZERO, "a fitted canvas does not pan");
    assert!(!m.can_pan());
    m.set_percent(200.0);
    m.pan_by(10_000.0, -10_000.0);
    let r = m.canvas_rect();
    assert!(close(r.x0, 0.0) && close(r.y1, 600.0), "{r:?}");
    m.fit();
    assert_eq!(m.pan, Point::ZERO);
    assert!(m.fit);
}

#[test]
fn the_viewport_model_is_what_the_engine_sees() {
    let (mut app, session) = open(700.0, 400.0, opened(1200, 640, vec![], Default::default()));
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(150.0, cx))
    });
    let (model, engine, revision) = app.cx.update(|cx| {
        let v = session.viewport.read(cx);
        (
            v.viewport(),
            *session.doc.read(cx).editor().viewport(),
            v.revision(),
        )
    });
    assert_eq!(model, engine);
    assert!(close(model.scale, 0.75));
    assert!(revision > 1);
}

fn open_source(source: EditorSource) -> Result<(u32, u32, usize), Error> {
    let (mut app, _) = open(200.0, 120.0, opened(4, 4, vec![], Default::default()));
    let slot: Rc<RefCell<Option<Result<EditorSession, Error>>>> = Rc::default();
    let out = slot.clone();
    app.cx.update(|cx| {
        let task = EditorSession::open(source, cx);
        cx.spawn(async move |_| {
            *out.borrow_mut() = Some(task.await);
        })
        .detach();
    });
    app.until("the session", |_| slot.borrow().is_some());
    let result = slot.borrow_mut().take().expect("opened");
    result.map(|session| {
        app.cx.update(|cx| {
            let d = session.doc.read(cx);
            (
                d.base().width(),
                d.base().height(),
                d.editor().doc().shapes().len(),
            )
        })
    })
}

#[test]
fn a_file_opens_with_its_pixels_and_an_empty_document() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("session-open");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("plain.png");
    base(64, 32).save(&file).unwrap();
    let opened = open_source(EditorSource::File(file.clone())).expect("a png opens");
    assert_eq!(opened, (64, 32, 0));
    let missing = open_source(EditorSource::File(dir.join("missing.png")));
    assert!(
        matches!(missing, Err(Error::Io { .. })),
        "{:?}",
        missing.err()
    );
    let staged = open_source(EditorSource::Staged(uuid::Uuid::nil()));
    assert!(matches!(staged, Err(Error::Unsupported)));
}
