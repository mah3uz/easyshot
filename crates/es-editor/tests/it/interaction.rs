#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::interaction_support;

use canvas_support::*;
use es_model::{GeoKind, RedactionKind, ShapeKind, Tool};
use gpui::{CursorStyle, Modifiers, point, px};
use interaction_support::*;

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn every_drawing_tool_makes_its_shape_where_the_mouse_dragged() {
    let (mut app, session) = open(900.0, 560.0, opened(1200, 640, vec![], Default::default()));
    zoomed(&mut app, &session);
    let tools = [
        Tool::Rectangle,
        Tool::FilledRectangle,
        Tool::Ellipse,
        Tool::Highlight,
        Tool::Blur,
        Tool::Pixelate,
        Tool::Line,
        Tool::Arrow,
        Tool::Freehand,
        Tool::NumberedCircle,
    ];
    for tool in tools {
        set_tool(&mut app, &session, tool);
        let before = ids(&app, &session);
        let (from, to) = (
            at_page(&app, &session, 300.0, 200.0),
            at_page(&app, &session, 420.0, 290.0),
        );
        drag(&mut app, from, to, 12);
        let made: Vec<_> = ids(&app, &session)
            .into_iter()
            .filter(|id| !before.contains(id))
            .collect();
        assert_eq!(made.len(), 1, "{tool:?} made one shape");
        let s = shape(&app, &session, made[0]);
        let b = page_bounds(&app, &session, made[0]);
        let boxed = |b: es_geom::BBox| {
            near(b.x, 300.0, 1.0)
                && near(b.y, 200.0, 1.0)
                && near(b.w, 120.0, 1.0)
                && near(b.h, 90.0, 1.0)
        };
        match (tool, &s.kind) {
            (Tool::Rectangle | Tool::FilledRectangle, ShapeKind::Geo(g)) => {
                assert_eq!(g.geo, GeoKind::Rectangle);
                assert!(boxed(b), "{tool:?} at {b:?}");
            }
            (Tool::Ellipse, ShapeKind::Geo(g)) => {
                assert_eq!(g.geo, GeoKind::Ellipse);
                let props = es_geom::BBox {
                    x: s.x,
                    y: s.y,
                    w: g.w,
                    h: g.h,
                };
                assert!(boxed(props), "{tool:?} at {props:?}");
            }
            (Tool::Highlight, ShapeKind::Highlight(_)) => assert!(boxed(b), "{tool:?} at {b:?}"),
            (Tool::Blur, ShapeKind::Redaction(r)) => {
                assert_eq!(r.kind, RedactionKind::Blur);
                assert!(boxed(b), "{tool:?} at {b:?}");
            }
            (Tool::Pixelate, ShapeKind::Redaction(r)) => {
                assert_eq!(r.kind, RedactionKind::Pixelate);
                assert!(boxed(b), "{tool:?} at {b:?}");
            }
            (Tool::Line | Tool::Arrow, ShapeKind::Arrow(a)) => {
                let start = (s.x + a.start.x, s.y + a.start.y);
                let end = (s.x + a.end.x, s.y + a.end.y);
                assert!(
                    near(start.0, 300.0, 1.0) && near(start.1, 200.0, 1.0),
                    "{start:?}"
                );
                assert!(
                    near(end.0, 420.0, 1.0) && near(end.1, 290.0, 1.0),
                    "{end:?}"
                );
            }
            (Tool::Freehand, ShapeKind::Draw(d)) => {
                assert!(d.is_complete && d.points.len() > 8);
                assert!(
                    near(b.x, 300.0, 6.0) && near(b.x + b.w, 420.0, 6.0),
                    "{b:?}"
                );
                assert!(
                    near(b.y, 200.0, 6.0) && near(b.y + b.h, 290.0, 6.0),
                    "{b:?}"
                );
            }
            (Tool::NumberedCircle, ShapeKind::Numbered(_)) => {
                let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
                assert!(near(cx, 300.0, 1.0) && near(cy, 200.0, 1.0), "{b:?}");
            }
            (tool, kind) => panic!("{tool:?} made {kind:?}"),
        }
    }
}

#[test]
fn a_live_stroke_retessellates_only_the_chunks_a_move_changed() {
    let (mut app, session) = open(900.0, 560.0, opened(1200, 640, vec![], Default::default()));
    set_tool(&mut app, &session, Tool::Freehand);
    let y = |i: usize| 320.0 + (i as f64 * 0.05).sin() * 40.0;
    let at = at_page(&app, &session, 100.0, y(0));
    down(&mut app, at);
    let c = canvas(&app);
    let mut most = 0;
    for i in 1..=600 {
        let x = 100.0 + i as f64 * 1.5;
        let at = at_page(&app, &session, x, y(i));
        drag_to(&mut app, at);
        let stats = c.read_with(&app.cx, |c, _| c.last_frame());
        assert_eq!(
            stats.shapes_tessellated, 0,
            "the live shape is painted from its chunks"
        );
        if i > 4 {
            most = most.max(stats.live_chunks_tessellated);
        }
    }
    assert!((1..=2).contains(&most), "a move rebuilt {most} chunks");
    let shot = app.screenshot("interaction-live-stroke");
    let tail = at_page(&app, &session, 100.0 + 590.0 * 1.5, y(590));
    let k = shot.width() as f32 / 900.0;
    let (tx, ty) = (
        (f32::from(tail.x) * k) as u32,
        (f32::from(tail.y) * k) as u32,
    );
    let px = shot.get_pixel(tx, ty);
    let base = shot.get_pixel(tx, ty + 40);
    let off: i32 = (0..3)
        .map(|k| (i32::from(px[k]) - i32::from(base[k])).abs())
        .sum();
    assert!(
        off > 120,
        "the stroke's newest end is painted: {px:?} over {base:?}"
    );

    let at = at_page(&app, &session, 100.0 + 600.0 * 1.5, y(600));

    up(&mut app, at);
    let id = ids(&app, &session)[0];
    match shape(&app, &session, id).kind {
        ShapeKind::Draw(d) => assert!(d.is_complete),
        other => panic!("{other:?}"),
    }
    let stats = c.read_with(&app.cx, |c, _| c.last_frame());
    assert_eq!(stats.live_chunks_tessellated, 0);
}

#[test]
fn handles_resize_and_rotate_the_selection() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![rect(300.0, 200.0, 300.0, 200.0)],
            Default::default(),
        ),
    );
    zoomed(&mut app, &session);
    set_tool(&mut app, &session, Tool::Select);
    let id = ids(&app, &session)[0];
    let on_edge = at_page(&app, &session, 300.0, 300.0);
    down(&mut app, on_edge);
    up(&mut app, on_edge);
    assert_eq!(selected(&app, &session), vec![id]);

    let before = page_bounds(&app, &session, id);
    let corner = at_page(&app, &session, 600.0, 400.0);
    let to = at_page(&app, &session, 660.0, 440.0);
    drag(&mut app, corner, to, 6);
    let after = page_bounds(&app, &session, id);
    assert!(
        near(after.w - before.w, 60.0, 1.0),
        "{before:?} -> {after:?}"
    );
    assert!(
        near(after.h - before.h, 40.0, 1.0),
        "{before:?} -> {after:?}"
    );
    assert!(near(after.x, before.x, 0.5) && near(after.y, before.y, 0.5));

    let centre = at_page(&app, &session, 480.0, 320.0);
    let top_right = at_page(&app, &session, 660.0, 200.0);
    let out = top_right - centre;
    let len = f32::from(out.x).hypot(f32::from(out.y));
    let grip = top_right + out * (14.0 / len);
    let radius = (f32::from(grip.x - centre.x)).hypot(f32::from(grip.y - centre.y));
    let start = f32::from(grip.y - centre.y).atan2(f32::from(grip.x - centre.x));
    down(&mut app, grip);
    for i in 1..=12 {
        let a = start + std::f32::consts::FRAC_PI_2 * i as f32 / 12.0;
        drag_to(
            &mut app,
            centre + point(px(radius * a.cos()), px(radius * a.sin())),
        );
    }
    let a = start + std::f32::consts::FRAC_PI_2;
    up(
        &mut app,
        centre + point(px(radius * a.cos()), px(radius * a.sin())),
    );
    let turned = shape(&app, &session, id)
        .rotation
        .rem_euclid(std::f64::consts::TAU);
    assert!(
        near(turned, std::f64::consts::FRAC_PI_2, 0.03),
        "rotation {turned}"
    );
}

#[test]
fn the_cursor_follows_the_tool_the_handle_and_the_gesture() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![rect(300.0, 200.0, 300.0, 200.0)],
            Default::default(),
        ),
    );
    let custom = app.cx.update(es_editor::cursor::cursors);
    let id = ids(&app, &session)[0];

    let at = at_page(&app, &session, 800.0, 500.0);

    hover(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::Custom(custom.rectangle));
    let at = at_page(&app, &session, 300.0, 300.0);
    hover(&mut app, at);
    assert_eq!(
        cursor(&app),
        CursorStyle::Custom(custom.rectangle),
        "a drawing tool keeps its cursor over shapes"
    );
    hover(&mut app, point(px(6.), px(6.)));
    assert_eq!(cursor(&app), CursorStyle::Arrow, "outside the frame");

    set_tool(&mut app, &session, Tool::Select);
    let at = at_page(&app, &session, 800.0, 500.0);
    hover(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::Arrow);
    let at = at_page(&app, &session, 300.0, 300.0);
    hover(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::OpenHand);

    select(&mut app, &session, vec![id]);
    let at = at_page(&app, &session, 600.0, 400.0);
    hover(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::ResizeUpLeftDownRight);
    let at = at_page(&app, &session, 450.0, 200.0);
    hover(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::ResizeUpDown);
    let corner = at_page(&app, &session, 600.0, 200.0);
    hover(&mut app, corner + point(px(10.), px(-10.)));
    assert_eq!(cursor(&app), CursorStyle::Custom(custom.rotate[7]));

    let at = at_page(&app, &session, 380.0, 400.0);
    down(&mut app, at);
    let at = at_page(&app, &session, 400.0, 410.0);
    drag_to(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::ClosedHand);
    up(&mut app, at);
    assert_eq!(cursor(&app), CursorStyle::OpenHand);
}

#[test]
fn a_marquee_selects_the_shapes_it_encloses() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![
                rect(100.0, 100.0, 100.0, 80.0),
                rect(300.0, 120.0, 100.0, 80.0),
                rect(700.0, 400.0, 100.0, 80.0),
            ],
            Default::default(),
        ),
    );
    zoomed(&mut app, &session);
    set_tool(&mut app, &session, Tool::Select);
    let all = ids(&app, &session);
    let from = at_page(&app, &session, 60.0, 60.0);
    let to = at_page(&app, &session, 450.0, 250.0);
    down(&mut app, from);
    drag_to(&mut app, to);
    assert_eq!(
        cursor(&app),
        CursorStyle::Arrow,
        "a marquee drag keeps the arrow"
    );
    up(&mut app, to);
    let mut chosen = selected(&app, &session);
    let mut want = all[..2].to_vec();
    chosen.sort();
    want.sort();
    assert_eq!(chosen, want);
}

#[test]
fn shift_drag_keeps_a_rectangle_square() {
    let (mut app, session) = open(900.0, 560.0, opened(1200, 640, vec![], Default::default()));
    let (from, to) = (
        at_page(&app, &session, 200.0, 200.0),
        at_page(&app, &session, 360.0, 260.0),
    );
    down_with(&mut app, from, Modifiers::shift());
    send(
        &mut app,
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: to,
            pressed_button: Some(gpui::MouseButton::Left),
            modifiers: Modifiers::shift(),
        }),
    );
    let at = at_page(&app, &session, 360.0, 260.0);
    up(&mut app, at);
    let b = page_bounds(&app, &session, ids(&app, &session)[0]);
    assert!(near(b.w, b.h, 1.0), "{b:?}");
}
