#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::interaction_support;

use canvas_support::*;
use es_geom::PVec;
use es_model::{AnnoShape, ArrowProps, Arrowhead, ShapeKind, Tool};
use es_ui::tokens::ThemeMode;
use gpui::{Pixels, point};
use image::RgbaImage;
use interaction_support::*;

const SELECTION: [u8; 3] = [0x31, 0x82, 0xed];

fn sample(shot: &RgbaImage, at: gpui::Point<Pixels>) -> [u8; 3] {
    let k = shot.width() as f32 / 900.0;
    let p = shot.get_pixel(
        (f32::from(at.x) * k).round() as u32,
        (f32::from(at.y) * k).round() as u32,
    );
    [p[0], p[1], p[2]]
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: i32) -> bool {
    (0..3).all(|i| (i32::from(a[i]) - i32::from(b[i])).abs() <= tolerance)
}

fn mix(base: [u8; 3], over: [u8; 3], alpha: f32) -> [u8; 3] {
    std::array::from_fn(|i| {
        (f32::from(base[i]) * (1.0 - alpha) + f32::from(over[i]) * alpha).round() as u8
    })
}

fn scene() -> Vec<AnnoShape> {
    let mut turned = rect(160.0, 140.0, 300.0, 180.0);
    turned.rotation = 0.25;
    vec![
        turned,
        AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Arrow(ArrowProps {
                start: PVec::new(620.0, 420.0),
                end: PVec::new(1000.0, 180.0),
                bend: 70.0,
                arrowhead_end: Arrowhead::Arrow,
                swatch: swatch(0.95, 0.35, 0.15),
                stroke_width: 6.0,
                ..ArrowProps::default()
            }),
        ),
    ]
}

fn page_point(app: &App, session: &es_editor::EditorSession, p: PVec) -> gpui::Point<Pixels> {
    at_page(app, session, p.x, p.y)
}

#[test]
fn a_selected_shape_shows_its_frame_and_corner_squares_in_either_theme() {
    for (mode, name, handle) in [
        (ThemeMode::Light, "light", [0xff, 0xff, 0xff]),
        (ThemeMode::Dark, "dark", [0x2a, 0x2a, 0x2c]),
    ] {
        let (mut app, session) = open(900.0, 560.0, opened(1200, 640, scene(), Default::default()));
        app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
        set_tool(&mut app, &session, Tool::Select);
        let id = ids(&app, &session)[0];
        select(&mut app, &session, vec![id]);
        let shot = app.screenshot(&format!("chrome-selection-{name}"));
        let corners = session.doc.read_with(&app.cx, |d, _| {
            d.editor()
                .selection_bounds()
                .expect("one shape is selected")
                .page_corners()
        });
        for c in corners {
            let at = page_point(&app, &session, c);
            assert!(
                close(sample(&shot, at), handle, 6),
                "{name}: the corner square at {at:?} is {:?}",
                sample(&shot, at)
            );
        }
        let edge = PVec::new(
            corners[0].x + (corners[1].x - corners[0].x) * 0.3,
            corners[0].y + (corners[1].y - corners[0].y) * 0.3,
        );
        let at = page_point(&app, &session, edge);
        assert!(
            close(sample(&shot, at), SELECTION, 40),
            "{name}: the frame at {at:?} is {:?}",
            sample(&shot, at)
        );
    }
}

#[test]
fn a_lone_arrow_shows_three_round_handles_on_a_dashed_guide() {
    let (mut app, session) = open(900.0, 560.0, opened(1200, 640, scene(), Default::default()));
    app.cx
        .update(|cx| es_ui::theme::set_mode(ThemeMode::Light, cx));
    set_tool(&mut app, &session, Tool::Select);
    let arrow = ids(&app, &session)[1];
    select(&mut app, &session, vec![arrow]);
    let shot = app.screenshot("chrome-arrow-light");
    let handles = session.doc.read_with(&app.cx, |d, _| {
        let doc = d.editor().doc();
        doc.arrow_info(arrow).expect("arrow resolves").handles()
    });
    for h in handles {
        let at = page_point(&app, &session, h);
        assert!(
            close(sample(&shot, at), [0xff, 0xff, 0xff], 6),
            "the handle at {at:?} is {:?}",
            sample(&shot, at)
        );
    }
    let frame = session
        .doc
        .read_with(&app.cx, |d, _| d.editor().selection_bounds())
        .expect("bounds")
        .page_corners();
    let mid_top = PVec::new((frame[0].x + frame[1].x) / 2.0, frame[0].y);
    let at = page_point(&app, &session, mid_top);
    assert!(
        !close(sample(&shot, at), SELECTION, 40),
        "a lone arrow draws no frame"
    );
}

#[test]
fn the_marquee_tints_what_it_covers_with_the_selection_fill() {
    for (mode, name, fill, alpha) in [
        (ThemeMode::Light, "light", [0x1f, 0x8f, 0xff], 0.24),
        (ThemeMode::Dark, "dark", [0x1f, 0x91, 0xff], 0.20),
    ] {
        let (mut app, session) = open(900.0, 560.0, opened(1200, 640, scene(), Default::default()));
        app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
        set_tool(&mut app, &session, Tool::Select);
        let inside = at_page(&app, &session, 160.0, 500.0);
        let before = sample(
            &app.screenshot(&format!("chrome-marquee-before-{name}")),
            inside,
        );
        let from = at_page(&app, &session, 60.0, 380.0);
        let to = at_page(&app, &session, 560.0, 600.0);
        down(&mut app, from);
        drag_to(&mut app, to);
        let shot = app.screenshot(&format!("chrome-marquee-{name}"));
        let want = mix(before, fill, alpha);
        assert!(
            close(sample(&shot, inside), want, 4),
            "{name}: {:?} over {before:?}, want {want:?}",
            sample(&shot, inside)
        );
        let edge = point(from.x, (from.y + to.y) / 2.0);
        assert!(
            close(sample(&shot, edge), SELECTION, 40),
            "{name}: the marquee line"
        );
        up(&mut app, to);
    }
}

#[test]
fn dragging_an_arrow_end_onto_a_shape_outlines_the_binding_target() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![rect(600.0, 200.0, 240.0, 160.0)],
            Default::default(),
        ),
    );
    app.cx
        .update(|cx| es_ui::theme::set_mode(ThemeMode::Light, cx));
    let target = ids(&app, &session)[0];
    set_tool(&mut app, &session, Tool::Arrow);
    let inside = at_page(&app, &session, 700.0, 300.0);
    let before = sample(&app.screenshot("chrome-hint-before"), inside);
    let from = at_page(&app, &session, 200.0, 450.0);
    down(&mut app, from);
    let at = at_page(&app, &session, 400.0, 380.0);
    drag_to(&mut app, at);
    let at = at_page(&app, &session, 720.0, 280.0);
    drag_to(&mut app, at);
    let hinted = session
        .doc
        .read_with(&app.cx, |d, _| d.editor().hinted_binding_id());
    assert_eq!(hinted, Some(target));
    let shot = app.screenshot("chrome-hint-light");
    let want = mix(before, [0x1f, 0x8f, 0xff], 0.24);
    assert!(
        close(sample(&shot, inside), want, 4),
        "{:?} over {before:?}, want {want:?}",
        sample(&shot, inside)
    );
    let at = at_page(&app, &session, 720.0, 280.0);
    up(&mut app, at);
}
