#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use canvas_support::*;
use es_anno::GestureId;
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::GradientPreset;
use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    ScrollDelta, ScrollEvent, point, px,
};

fn differing(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    a.pixels().zip(b.pixels()).filter(|(p, q)| p != q).count()
}

#[test]
fn every_shape_kind_paints_over_the_image() {
    let (mut bare, _) = open(760.0, 420.0, opened(1200, 640, vec![], Default::default()));
    let empty = bare.screenshot("canvas-empty");
    drop(bare);
    let (mut app, _) = open(
        760.0,
        420.0,
        opened(1200, 640, every_kind(true, true), Default::default()),
    );
    let full = app.screenshot("canvas-every-kind");
    let stats = canvas(&app).read_with(&app.cx, |c, _| c.last_frame());
    assert!(stats.paths_painted >= 8, "{stats:?}");
    assert!(
        differing(&empty, &full) > 40_000,
        "the shapes barely changed the frame"
    );
    let workspace = full.get_pixel(4, 4);
    assert_eq!(empty.get_pixel(4, 4), workspace);
}

#[test]
fn an_unchanged_frame_reuses_every_path_and_a_pan_only_moves_them() {
    let (mut app, session) = open(
        760.0,
        420.0,
        opened(1200, 640, every_kind(false, true), Default::default()),
    );
    let c = canvas(&app);
    redraw(&mut app);
    let still = c.read_with(&app.cx, |c, _| c.last_frame());
    assert_eq!((still.shapes_tessellated, still.shapes_placed), (0, 0));

    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(150.0, cx))
    });
    app.settle();
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.pan_by(-40.0, 0.0, cx))
    });
    app.settle();
    let panned = c.read_with(&app.cx, |c, _| c.last_frame());
    assert_eq!(panned.shapes_tessellated, 0, "a pan must not re-tessellate");
    assert!(panned.shapes_placed > 0);
}

#[test]
fn an_edited_shape_is_tessellated_again_and_moves_on_screen() {
    let (mut app, session) = open(
        760.0,
        420.0,
        opened(
            1200,
            640,
            vec![rect(100.0, 100.0, 300.0, 200.0)],
            Default::default(),
        ),
    );
    let before = app.screenshot("canvas-edit-before");
    app.cx.update(|cx| {
        session.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_all());
            d.apply(cx, |e| e.nudge_selected(80.0, 40.0, GestureId(1)));
        })
    });
    let after = app.screenshot("canvas-edit-after");
    let stats = canvas(&app).read_with(&app.cx, |c, _| c.last_frame());
    assert_eq!(stats.shapes_tessellated, 1);
    assert!(differing(&before, &after) > 2_000);
}

#[test]
fn zoomed_in_the_canvas_paints_the_shapes_larger_and_sharper() {
    let (mut app, session) = open(
        760.0,
        420.0,
        opened(1200, 640, every_kind(false, true), Default::default()),
    );
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(250.0, cx))
    });
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.pan_by(1000.0, 1000.0, cx))
    });
    app.screenshot("canvas-zoom-250");
    let (percent, vp) = app.cx.update(|cx| {
        let v = session.viewport.read(cx);
        (v.zoom_percent(), v.viewport())
    });
    assert_eq!(percent, 250.0);
    assert_eq!(vp.scale, 1.25);
    assert_eq!(vp.origin.x, 0.0, "panned to the top-left corner");
}

#[test]
fn a_gradient_look_draws_its_background_around_the_card() {
    let look = BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-lilac").unwrap().style()),
        ..BackgroundSettings::default()
    };
    let (mut app, session) = open(
        760.0,
        420.0,
        opened(1200, 640, every_kind(false, true), look),
    );
    let c = canvas(&app);
    app.until("the underlay", |cx| c.read(cx).underlay_ready());
    let shot = app.screenshot("canvas-gradient");
    let (canvas_rect, image) = app.cx.update(|cx| {
        let v = session.viewport.read(cx);
        let vp = v.viewport();
        (v.math().canvas_rect(), vp.origin)
    });
    assert!(image.x - canvas_rect.x0 > 20.0, "the look adds padding");
    let device = |x: f64, y: f64| shot.get_pixel((x * 2.0) as u32, (y * 2.0) as u32).0;
    let pad = device(canvas_rect.x0 + 6.0, canvas_rect.y0 + 6.0);
    let workspace = device(2.0, 2.0);
    assert_ne!(
        pad, workspace,
        "the padding shows the gradient, not the workspace"
    );
    let [r, g, b, _] = pad;
    assert!(r > 150 && b > 150, "soft lilac is light: {pad:?}");
    assert!(g < r.max(b), "and tinted: {pad:?}");
}

#[test]
fn only_the_tiles_in_view_are_painted() {
    let (mut app, session) = open(600.0, 400.0, opened(9000, 300, vec![], Default::default()));
    let c = canvas(&app);
    assert_eq!(c.read_with(&app.cx, |c, _| c.last_frame().tiles_painted), 3);
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(100.0, cx))
    });
    app.settle();
    assert_eq!(c.read_with(&app.cx, |c, _| c.last_frame().tiles_painted), 2);
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.pan_by(1950.0, 0.0, cx))
    });
    app.settle();
    assert_eq!(c.read_with(&app.cx, |c, _| c.last_frame().tiles_painted), 1);
}

#[test]
fn ctrl_wheel_zooms_about_the_pointer_and_touchpad_scroll_pans() {
    let (mut app, session) = open(760.0, 420.0, opened(1200, 640, vec![], Default::default()));
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(200.0, cx))
    });
    app.settle();
    let pointer = point(px(500.), px(300.));
    let page_at = |app: &mut App| {
        app.cx.update(|cx| {
            let vp = session.viewport.read(cx).viewport();
            vp.screen_to_page(es_geom::Point::new(500.0, 300.0))
        })
    };
    let before = page_at(&mut app);
    let ctrl = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    send(
        &mut app,
        PlatformInput::Scroll(ScrollEvent {
            position: pointer,
            delta: ScrollDelta::Pixels(point(px(0.), px(120.))),
            modifiers: ctrl,
            ..ScrollEvent::default()
        }),
    );
    let percent = app.cx.update(|cx| session.viewport.read(cx).zoom_percent());
    assert_eq!(percent, (200.0f64 * 1.3).round());
    let after = page_at(&mut app);
    assert!(
        (before.x - after.x).abs() < 1e-6 && (before.y - after.y).abs() < 1e-6,
        "{before:?} drifted to {after:?}"
    );

    let origin = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    send(
        &mut app,
        PlatformInput::Scroll(ScrollEvent {
            position: pointer,
            delta: ScrollDelta::Pixels(point(px(-30.), px(-20.))),
            ..ScrollEvent::default()
        }),
    );
    let moved = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    assert_eq!((moved.x - origin.x, moved.y - origin.y), (-30.0, -20.0));
}

#[test]
fn a_wheel_notch_zooms_about_the_pointer_without_a_modifier_and_scroll_pixels_do_not() {
    let (mut app, session) = open(760.0, 420.0, opened(1200, 640, vec![], Default::default()));
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(200.0, cx))
    });
    app.settle();
    let pointer = point(px(500.), px(300.));
    let page_at = |app: &mut App| {
        app.cx.update(|cx| {
            let vp = session.viewport.read(cx).viewport();
            vp.screen_to_page(es_geom::Point::new(500.0, 300.0))
        })
    };
    let before = page_at(&mut app);
    send(
        &mut app,
        PlatformInput::Scroll(ScrollEvent {
            position: pointer,
            delta: ScrollDelta::Lines(point(0., 6.)),
            ..ScrollEvent::default()
        }),
    );
    let percent = app.cx.update(|cx| session.viewport.read(cx).zoom_percent());
    assert_eq!(percent, (200.0f64 * 1.3).round());
    let after = page_at(&mut app);
    assert!((before.x - after.x).abs() < 1e-6 && (before.y - after.y).abs() < 1e-6);

    send(
        &mut app,
        PlatformInput::Scroll(ScrollEvent {
            position: pointer,
            delta: ScrollDelta::Pixels(point(px(0.), px(120.))),
            ..ScrollEvent::default()
        }),
    );
    let still = app.cx.update(|cx| session.viewport.read(cx).zoom_percent());
    assert_eq!(still, percent);
}

#[test]
fn a_middle_button_drag_pans_the_canvas() {
    let (mut app, session) = open(760.0, 420.0, opened(1200, 640, vec![], Default::default()));
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(200.0, cx))
    });
    app.settle();
    let origin = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    let at = |x: f32, y: f32| point(px(x), px(y));
    send(
        &mut app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Middle,
            position: at(400., 200.),
            click_count: 1,
            ..MouseDownEvent::default()
        }),
    );
    send(
        &mut app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: at(350., 230.),
            pressed_button: Some(MouseButton::Middle),
            ..MouseMoveEvent::default()
        }),
    );
    send(
        &mut app,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Middle,
            position: at(350., 230.),
            click_count: 1,
            ..MouseUpEvent::default()
        }),
    );
    let moved = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    assert_eq!((moved.x - origin.x, moved.y - origin.y), (-50.0, 30.0));
    let panning = canvas(&app).read_with(&app.cx, |c, _| c.is_panning());
    assert!(!panning);
}
