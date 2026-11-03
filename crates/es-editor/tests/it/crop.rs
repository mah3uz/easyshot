#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::interaction_support;

use std::{cell::RefCell, f64::consts::FRAC_PI_6, rc::Rc, sync::Arc};

use canvas_support::{App, TEXT, canvas, open, opened, rect, redraw, send};
use es_anno::crop::{AspectPreset, CropHandle, ImageOp};
use es_editor::session::crop::{CropModel, CropTarget, IMAGE_UNDO_STEPS};
use es_editor::{EditorSession, EditorWindow};
use es_geom::{PVec, Point};
use es_model::{AnnoShape, ArrowProps, BackgroundSettings, ShapeKind};
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Context, CursorStyle, Entity, KeyDownEvent, Keystroke, PlatformInput, point, px,
};
use gpui_component::Root;
use image::RgbaImage;
use interaction_support::{at_page, down, drag_to, hover, up};

fn scene() -> Vec<AnnoShape> {
    let mut turned = rect(300.0, 200.0, 100.0, 80.0);
    turned.rotation = FRAC_PI_6;
    vec![
        turned,
        rect(700.0, 380.0, 160.0, 90.0),
        AnnoShape::new(
            40.0,
            30.0,
            ShapeKind::Arrow(ArrowProps {
                start: PVec::new(20.0, 400.0),
                end: PVec::new(500.0, 120.0),
                bend: 40.0,
                stroke_width: 5.0,
                ..ArrowProps::default()
            }),
        ),
    ]
}

fn session_app(width: u32, height: u32) -> (App, EditorSession) {
    open(
        900.0,
        600.0,
        opened(width, height, scene(), BackgroundSettings::default()),
    )
}

fn crop<R>(
    app: &mut App,
    s: &EditorSession,
    f: impl FnOnce(&mut CropModel, &mut Context<CropModel>) -> R,
) -> R {
    let out = app.cx.update(|cx| s.crop.update(cx, f));
    app.settle();
    out
}

fn base(app: &App, s: &EditorSession) -> Arc<RgbaImage> {
    s.doc.read_with(&app.cx, |d, _| d.base().clone())
}

fn shapes(app: &App, s: &EditorSession) -> Vec<AnnoShape> {
    s.doc
        .read_with(&app.cx, |d, _| d.editor().doc().snapshot().shapes)
}

fn same_shapes(a: &[AnnoShape], b: &[AnnoShape]) -> bool {
    let close = |x: f64, y: f64| (x - y).abs() < 1e-9;
    a.len() == b.len()
        && a.iter().zip(b).all(|(p, q)| {
            p.id == q.id
                && close(p.x, q.x)
                && close(p.y, q.y)
                && close(p.rotation.sin(), q.rotation.sin())
                && close(p.rotation.cos(), q.rotation.cos())
                && p.mirrored == q.mirrored
                && p.kind == q.kind
        })
}

fn drag_handle(app: &mut App, s: &EditorSession, handle: CropHandle, from: Point, to: Point) {
    crop(app, s, |c, cx| {
        c.press(CropTarget::Handle(handle), from);
        c.drag_to(to, false, cx);
        c.release(cx);
    });
}

fn press_key(app: &mut App, keystroke: &'static str) {
    send(
        app,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(keystroke).expect("a valid keystroke"),
            is_held: false,
            prefer_character_input: false,
        }),
    );
}

#[test]
fn a_crop_sizes_the_base_to_the_rect_and_moves_every_annotation_by_its_origin() {
    let (mut app, s) = session_app(1200, 640);
    let before = shapes(&app, &s);
    let original = base(&app, &s);
    crop(&mut app, &s, |c, cx| c.begin(cx));
    drag_handle(
        &mut app,
        &s,
        CropHandle::TopLeft,
        Point::ZERO,
        Point::new(250.0, 150.0),
    );
    drag_handle(
        &mut app,
        &s,
        CropHandle::BottomRight,
        Point::new(1200.0, 640.0),
        Point::new(850.0, 500.0),
    );
    assert!(crop(&mut app, &s, |c, cx| c.apply(cx)));

    let cropped = base(&app, &s);
    assert_eq!(cropped.dimensions(), (600, 350));
    assert_eq!(cropped.get_pixel(0, 0), original.get_pixel(250, 150));
    assert_eq!(cropped.get_pixel(599, 349), original.get_pixel(849, 499));
    let after = shapes(&app, &s);
    for (b, a) in before.iter().zip(&after) {
        assert_eq!(
            (a.x, a.y),
            (b.x - 250.0, b.y - 150.0),
            "every shape keeps its pixels"
        );
        assert_eq!(a.rotation, b.rotation);
    }
    assert_eq!(
        after[0].kind, before[0].kind,
        "a whole-pixel crop does not rescale"
    );
    let image = s
        .viewport
        .read_with(&app.cx, |v, _| v.viewport().image_size);
    assert_eq!(
        (image.width, image.height),
        (600.0, 350.0),
        "the view lays out the new size"
    );
    assert!(!s.crop.read_with(&app.cx, |c, _| c.is_cropping()));
}

#[test]
fn a_locked_16_9_crop_keeps_its_ratio_under_a_pointer_drag_of_its_corner() {
    let (mut app, s) = session_app(1200, 640);
    crop(&mut app, &s, |c, cx| {
        c.begin(cx);
        c.set_aspect(AspectPreset::Landscape16x9, cx);
    });
    redraw(&mut app);
    let r = s.crop.read_with(&app.cx, |c, _| c.state().unwrap().rect());
    assert!((r.width() / r.height() - 16.0 / 9.0).abs() < 1e-9);

    let corner = at_page(&app, &s, r.x1, r.y1);
    hover(&mut app, corner);
    let cursor = canvas(&app).read_with(&app.cx, |c, _| c.cursor());
    assert_eq!(cursor, CursorStyle::ResizeUpLeftDownRight);
    down(&mut app, corner + point(px(-4.), px(3.)));
    let to = at_page(&app, &s, 700.0, 600.0);
    drag_to(&mut app, to);
    let to = at_page(&app, &s, 700.0, 600.0);
    up(&mut app, to);

    let after = s.crop.read_with(&app.cx, |c, _| c.state().unwrap().rect());
    assert!(
        after.width() < r.width() - 100.0,
        "the drag resized the rect: {after:?}"
    );
    assert!((after.width() / after.height() - 16.0 / 9.0).abs() < 1e-9);
    assert_eq!(
        (after.x0, after.y0),
        (r.x0, r.y0),
        "the opposite corner stays put"
    );
    let (w, h) = s
        .crop
        .read_with(&app.cx, |c, _| c.state().unwrap().output_size());
    assert_eq!(
        (w, h),
        (after.width().round() as u32, after.height().round() as u32)
    );
}

#[test]
fn rotating_left_then_right_restores_every_pixel_and_shape() {
    let (mut app, s) = session_app(1200, 640);
    let (pixels, before) = (base(&app, &s), shapes(&app, &s));
    assert!(crop(&mut app, &s, |c, cx| c.transform(ImageOp::RotateLeft, cx)));
    assert_eq!(base(&app, &s).dimensions(), (640, 1200));
    assert!(
        !same_shapes(&before, &shapes(&app, &s)),
        "the turn moved the shapes"
    );
    crop(&mut app, &s, |c, cx| c.transform(ImageOp::RotateRight, cx));
    assert_eq!(base(&app, &s).as_raw(), pixels.as_raw());
    assert!(same_shapes(&before, &shapes(&app, &s)));
}

#[test]
fn flipping_twice_on_either_axis_is_the_identity() {
    let (mut app, s) = session_app(1200, 640);
    let (pixels, before) = (base(&app, &s), shapes(&app, &s));
    for op in [ImageOp::FlipHorizontal, ImageOp::FlipVertical] {
        crop(&mut app, &s, |c, cx| c.transform(op, cx));
        assert_ne!(base(&app, &s).as_raw(), pixels.as_raw());
        assert!(
            shapes(&app, &s).iter().all(|sh| sh.mirrored),
            "{op:?} mirrors"
        );
        crop(&mut app, &s, |c, cx| c.transform(op, cx));
        assert_eq!(base(&app, &s).as_raw(), pixels.as_raw(), "{op:?}");
        assert!(same_shapes(&before, &shapes(&app, &s)), "{op:?}");
    }
}

#[test]
fn undo_walks_back_through_later_edits_then_the_crop_and_redo_comes_back() {
    let (mut app, s) = session_app(1200, 640);
    let (pixels, before) = (base(&app, &s), shapes(&app, &s));
    crop(&mut app, &s, |c, cx| c.begin(cx));
    drag_handle(
        &mut app,
        &s,
        CropHandle::TopLeft,
        Point::ZERO,
        Point::new(200.0, 100.0),
    );
    crop(&mut app, &s, |c, cx| c.apply(cx));
    let (cropped, moved) = (base(&app, &s), shapes(&app, &s));
    app.cx.update(|cx| {
        s.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_all());
            d.apply(cx, |e| e.delete_selected());
        })
    });
    assert!(shapes(&app, &s).is_empty());

    app.cx.update(|cx| s.undo(cx));
    assert_eq!(
        shapes(&app, &s),
        moved,
        "the delete is undone first, over the cropped image"
    );
    assert_eq!(base(&app, &s).dimensions(), cropped.dimensions());
    app.cx.update(|cx| s.undo(cx));
    assert_eq!(base(&app, &s).as_raw(), pixels.as_raw(), "then the crop");
    assert_eq!(shapes(&app, &s), before);
    assert!(!app.cx.update(|cx| s.can_undo(cx)));

    app.cx.update(|cx| s.redo(cx));
    assert_eq!(base(&app, &s).as_raw(), cropped.as_raw());
    assert_eq!(shapes(&app, &s), moved);
    app.cx.update(|cx| s.redo(cx));
    assert!(shapes(&app, &s).is_empty());

    app.cx.update(|cx| {
        s.undo(cx);
        s.undo(cx);
    });
    app.cx
        .update(|cx| s.doc.update(cx, |d, cx| d.apply(cx, |e| e.select_all())));
    app.cx.update(|cx| {
        s.doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.nudge_selected(1.0, 0.0, es_anno::GestureId(9)))
        })
    });
    assert!(
        !app.cx.update(|cx| s.can_redo(cx)),
        "a new edit after undoing the crop drops the crop's redo"
    );
}

#[test]
fn escape_and_the_toggle_leave_crop_mode_without_changing_anything() {
    let (mut app, s) = session_app(1200, 640);
    let (revision, before) = (
        s.doc.read_with(&app.cx, |d, _| d.base_revision()),
        shapes(&app, &s),
    );
    let focus = canvas(&app).read_with(&app.cx, |c, _| c.focus_handle().clone());
    app.cx
        .update_window(app.window.into(), |_, w, cx| w.focus(&focus, cx))
        .unwrap();
    crop(&mut app, &s, |c, cx| c.begin(cx));
    redraw(&mut app);
    let r = s.crop.read_with(&app.cx, |c, _| c.state().unwrap().rect());
    let to = at_page(&app, &s, r.x0, r.y0);
    down(&mut app, to);
    let to = at_page(&app, &s, 300.0, 200.0);
    drag_to(&mut app, to);
    let to = at_page(&app, &s, 300.0, 200.0);
    up(&mut app, to);
    assert!(
        !s.crop
            .read_with(&app.cx, |c, _| c.state().unwrap().is_full())
    );

    press_key(&mut app, "escape");
    assert!(!s.crop.read_with(&app.cx, |c, _| c.is_cropping()));
    crop(&mut app, &s, |c, cx| c.toggle(cx));
    assert!(s.crop.read_with(&app.cx, |c, _| c.is_cropping()));
    crop(&mut app, &s, |c, cx| c.toggle(cx));
    assert!(!s.crop.read_with(&app.cx, |c, _| c.is_cropping()));
    assert_eq!(s.doc.read_with(&app.cx, |d, _| d.base_revision()), revision);
    assert_eq!(shapes(&app, &s), before);
    assert!(!app.cx.update(|cx| s.can_undo(cx)));
}

#[test]
fn replaced_base_tiles_are_released_and_image_steps_stay_bounded() {
    let (mut app, s) = session_app(5000, 300);
    redraw(&mut app);
    let stats = |app: &App| canvas(app).read_with(&app.cx, |c, _| c.last_frame());
    let first = stats(&app);
    assert_eq!((first.tiles_painted, first.images_released), (2, 0));
    crop(&mut app, &s, |c, cx| c.transform(ImageOp::RotateRight, cx));
    redraw(&mut app);
    assert!(
        stats(&app).images_released >= 2,
        "both old tiles went back: {:?}",
        stats(&app)
    );

    for _ in 0..IMAGE_UNDO_STEPS + 4 {
        crop(&mut app, &s, |c, cx| c.transform(ImageOp::FlipVertical, cx));
    }
    let (held, mut steps) = (s.crop.read_with(&app.cx, |c, _| c.held_bytes()), 0);
    assert_eq!(held, IMAGE_UNDO_STEPS * 5000 * 300 * 4);
    while app.cx.update(|cx| s.can_undo(cx)) {
        app.cx.update(|cx| s.undo(cx));
        steps += 1;
    }
    assert_eq!(steps, IMAGE_UNDO_STEPS, "the oldest steps were dropped");
}

type WindowApp = Headless<Root>;

fn launch(mode: ThemeMode) -> (WindowApp, Entity<EditorWindow>) {
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        1080.0,
        720.0,
        move |window, cx| {
            let doc = opened(1200, 640, scene(), BackgroundSettings::default());
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    (app, editor)
}

fn window_point(
    app: &WindowApp,
    editor: &Entity<EditorWindow>,
    p: Point,
) -> gpui::Point<gpui::Pixels> {
    editor.read_with(&app.cx, |w, cx| {
        let s = w.session().unwrap();
        let origin = w.canvas().unwrap().read(cx).bounds().origin;
        let at = s.viewport.read(cx).viewport().page_to_screen(p);
        origin + point(px(at.x as f32), px(at.y as f32))
    })
}

fn luma(img: &RgbaImage, at: gpui::Point<gpui::Pixels>) -> f32 {
    let sf = img.width() as f32 / 1080.0;
    let p = img.get_pixel((f32::from(at.x) * sf) as u32, (f32::from(at.y) * sf) as u32);
    (f32::from(p[0]) + f32::from(p[1]) + f32::from(p[2])) / 3.0
}

fn with_crop(
    app: &mut WindowApp,
    editor: &Entity<EditorWindow>,
    f: impl FnOnce(&mut CropModel, &mut Context<CropModel>),
) {
    let crop = app
        .cx
        .update(|cx| editor.read(cx).session().unwrap().crop.clone());
    app.cx.update(|cx| crop.update(cx, f));
    app.settle();
}

#[test]
fn crop_mode_dims_the_outside_and_frames_the_rect_with_white_handles_in_either_theme() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let (mut app, editor) = launch(mode);
        let plain = app.screenshot(&format!("crop-before-{name}"));
        with_crop(&mut app, &editor, |c, cx| {
            c.begin(cx);
            c.press(CropTarget::Handle(CropHandle::TopLeft), Point::ZERO);
            c.drag_to(Point::new(240.0, 120.0), false, cx);
            c.press(
                CropTarget::Handle(CropHandle::BottomRight),
                Point::new(1200.0, 640.0),
            );
            c.drag_to(Point::new(1000.0, 560.0), false, cx);
            c.release(cx);
        });
        let shot = app.screenshot(&format!("crop-mode-{name}"));
        let outside = window_point(&app, &editor, Point::new(1100.0, 600.0));
        let inside = window_point(&app, &editor, Point::new(620.0, 250.0));
        assert!(
            luma(&shot, outside) < luma(&plain, outside) * 0.6,
            "{name}: the outside is dimmed"
        );
        assert!(
            (luma(&shot, inside) - luma(&plain, inside)).abs() < 2.0,
            "{name}: the inside is not"
        );
        let corner = window_point(&app, &editor, Point::new(240.0, 120.0));
        assert!(luma(&shot, corner) > 245.0, "{name}: a white corner handle");
        assert!(app.cx.update(|cx| editor.read(cx).is_cropping(cx)));

        if mode == ThemeMode::Light {
            with_crop(&mut app, &editor, |c, cx| {
                c.apply(cx);
            });
            app.screenshot("crop-applied-light");
            with_crop(&mut app, &editor, |c, cx| {
                c.transform(ImageOp::RotateRight, cx);
            });
            app.screenshot("crop-rotated-light");
        }
    }
}
