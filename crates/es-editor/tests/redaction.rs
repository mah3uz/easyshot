#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod interaction_support;

use canvas_support::*;
use es_model::{AnnoShape, RedactionKind, RedactionProps, ShapeKind, Tool};
use gpui::{MouseButton, MouseMoveEvent, PlatformInput};
use image::RgbaImage;
use interaction_support::*;

fn redaction(x: f64, y: f64, w: f64, h: f64, kind: RedactionKind) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Redaction(RedactionProps {
            kind,
            w,
            h,
            density: 0.5,
        }),
    )
}

fn painted(app: &App) -> Vec<u64> {
    canvas(app).read_with(&app.cx, |c, _| c.redactions().painted().to_vec())
}

/// Black and white 6 px squares: any blur or pixel block turns them a flat grey.
fn checkerboard(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        if (x / 6 + y / 6) % 2 == 0 {
            image::Rgba([0, 0, 0, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    })
}

/// The spread of luminance over a page rectangle of the screenshot.
fn spread(shot: &RgbaImage, app: &App, session: &es_editor::EditorSession, r: [f64; 4]) -> f64 {
    let k = shot.width() as f32 / 900.0;
    let a = at_page(app, session, r[0], r[1]);
    let b = at_page(app, session, r[2], r[3]);
    let (x0, y0) = ((f32::from(a.x) * k) as u32, (f32::from(a.y) * k) as u32);
    let (x1, y1) = ((f32::from(b.x) * k) as u32, (f32::from(b.y) * k) as u32);
    let values: Vec<f64> = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(shot.get_pixel(x, y)[1]))
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64).sqrt()
}

#[test]
fn blur_and_pixelate_patches_cover_their_regions_and_nothing_else() {
    let doc = es_editor::Opened {
        base: checkerboard(1200, 640),
        ..opened(
            1200,
            640,
            vec![
                redaction(140.0, 140.0, 320.0, 220.0, RedactionKind::Blur),
                redaction(620.0, 140.0, 320.0, 220.0, RedactionKind::Pixelate),
            ],
            Default::default(),
        )
    };
    let (mut app, session) = open(900.0, 560.0, doc);
    let c = canvas(&app);
    app.until("both patches on screen", |cx| {
        c.read(cx).redactions().painted().len() == 2
    });
    let shot = app.screenshot("redaction-blur-pixelate");
    let outside = spread(&shot, &app, &session, [160.0, 400.0, 440.0, 600.0]);
    let blurred = spread(&shot, &app, &session, [180.0, 180.0, 420.0, 320.0]);
    let pixelated = spread(&shot, &app, &session, [660.0, 180.0, 900.0, 320.0]);
    assert!(outside > 40.0, "the bare checkerboard varies: {outside:.1}");
    assert!(blurred < 6.0, "the blur flattens it: {blurred:.1}");
    assert!(
        pixelated < 12.0,
        "the pixel blocks flatten it: {pixelated:.1}"
    );
    let held = c.read_with(&app.cx, |c, _| {
        (c.redactions().held(), c.redactions().building())
    });
    assert_eq!(held, (2, 0), "one patch per redaction while idle");
}

#[test]
fn a_dragged_redaction_keeps_its_old_patch_until_the_new_one_lands() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![redaction(300.0, 200.0, 240.0, 160.0, RedactionKind::Blur)],
            Default::default(),
        ),
    );
    let c = canvas(&app);
    app.until("a patch", |cx| c.read(cx).redactions().painted().len() == 1);
    set_tool(&mut app, &session, Tool::Select);
    let id = ids(&app, &session)[0];
    select(&mut app, &session, vec![id]);
    let grab = at_page(&app, &session, 420.0, 280.0);
    down(&mut app, grab);

    let handle = app.window.into();
    for step in 1..=5 {
        let first = painted(&app);
        let to = at_page(&app, &session, 420.0 + 20.0 * f64::from(step), 280.0);
        app.cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: to,
                        pressed_button: Some(MouseButton::Left),
                        ..MouseMoveEvent::default()
                    }),
                    cx,
                );
                window.draw(cx).clear(cx);
            })
            .expect("window is open");
        let kept = painted(&app);
        assert_eq!(
            kept, first,
            "step {step}: the old patch stays up while the new one builds"
        );
        let (held, building) = c.read_with(&app.cx, |c, _| {
            (c.redactions().held(), c.redactions().building())
        });
        assert!(
            held + building <= 2,
            "step {step}: {held} held, {building} building"
        );
        app.until("the new patch", |cx| {
            c.read(cx)
                .redactions()
                .painted()
                .first()
                .is_some_and(|id| first.first().is_some_and(|old| id > old))
        });
    }
    let end = at_page(&app, &session, 520.0, 280.0);
    up(&mut app, end);
    app.screenshot("redaction-blur-after-drag");
    let held = c.read_with(&app.cx, |c, _| {
        (c.redactions().held(), c.redactions().building())
    });
    assert_eq!(held, (1, 0));
}

#[test]
fn deleting_the_last_redaction_releases_its_patch() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(
            1200,
            640,
            vec![redaction(
                300.0,
                200.0,
                240.0,
                160.0,
                RedactionKind::Pixelate,
            )],
            Default::default(),
        ),
    );
    let c = canvas(&app);
    app.until("a patch", |cx| c.read(cx).redactions().held() == 1);
    app.cx.update(|cx| {
        session.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_all());
            d.apply(cx, |e| e.delete_selected());
        })
    });
    app.settle();
    let left = c.read_with(&app.cx, |c, _| {
        (c.redactions().held(), c.redactions().painted().len())
    });
    assert_eq!(left, (0, 0));
}
