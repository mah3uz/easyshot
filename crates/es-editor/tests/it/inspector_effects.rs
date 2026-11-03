#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::{cell::RefCell, rc::Rc};

use canvas_support::{TEXT, opened, rect};
use es_editor::{
    EditorSession, EditorWindow,
    inspector::{
        Section,
        blur::{BlurBody, BlurControl},
        camera::{CameraBody, CameraControl},
    },
    session::look::{LookEvent, LookModel},
};
use es_model::{BackgroundSettings, BlurEdgeMode, BlurMode};
use es_testkit::Headless;
use es_ui::{
    components::{InspectorSlider, Interaction},
    tokens::ThemeMode,
};
use gpui::{
    AppContext, Bounds, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PlatformInput, Point, point, px,
};
use gpui_component::Root;

const VALUE_FIELD: f32 = 60.;
const GAP: f32 = 7.;

struct Rig {
    app: Headless<Root>,
    look: Entity<LookModel>,
    camera: Entity<CameraBody>,
    blur: Entity<BlurBody>,
}

fn rig(mode: ThemeMode, look: BackgroundSettings) -> Rig {
    let doc = opened(1200, 640, vec![rect(40.0, 40.0, 300.0, 200.0)], look);
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::components::gallery::init(cx);
            es_ui::theme::set_mode(mode, cx);
            es_ui::motion::set_reduce_motion(true, cx);
        },
        1080.0,
        1260.0,
        move |window, cx| {
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot.borrow_mut().take().unwrap();
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    let (look, camera, blur) = app.cx.update(|cx| {
        let inspector = editor.read(cx).inspector().expect("an inspector");
        inspector.update(cx, |i, cx| {
            i.set_expanded(Section::Background, false, cx);
            i.set_expanded(Section::Camera, true, cx);
            i.set_expanded(Section::Blur, true, cx);
        });
        let i = inspector.read(cx);
        let camera = i.body(Section::Camera).expect("camera body").clone();
        let blur = i.body(Section::Blur).expect("blur body").clone();
        (
            i.look().clone(),
            camera.downcast::<CameraBody>().unwrap(),
            blur.downcast::<BlurBody>().unwrap(),
        )
    });
    app.cx.update(|cx| editor.update(cx, |_, cx| cx.notify()));
    app.settle();
    Rig {
        app,
        look,
        camera,
        blur,
    }
}

fn record(rig: &mut Rig) -> Rc<RefCell<Vec<LookEvent>>> {
    let seen: Rc<RefCell<Vec<LookEvent>>> = Rc::default();
    let sink = seen.clone();
    let look = rig.look.clone();
    rig.app.cx.update(|cx| {
        cx.subscribe(&look, move |_, e: &LookEvent, _| sink.borrow_mut().push(*e))
            .detach();
    });
    seen
}

fn settings(rig: &mut Rig) -> BackgroundSettings {
    rig.app.cx.update(|cx| rig.look.read(cx).settings().clone())
}

fn mouse(rig: &mut Rig, event: PlatformInput) {
    let handle = rig.app.window.into();
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
        .unwrap();
    rig.app.settle();
}

fn down(rig: &mut Rig, at: Point<Pixels>, clicks: usize) {
    mouse(
        rig,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            click_count: clicks,
            ..MouseDownEvent::default()
        }),
    );
}

fn move_to(rig: &mut Rig, at: Point<Pixels>) {
    mouse(
        rig,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: at,
            pressed_button: Some(MouseButton::Left),
            ..MouseMoveEvent::default()
        }),
    );
}

fn up(rig: &mut Rig, at: Point<Pixels>) {
    mouse(
        rig,
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: at,
            click_count: 1,
            ..MouseUpEvent::default()
        }),
    );
}

fn click(rig: &mut Rig, at: Point<Pixels>) {
    down(rig, at, 1);
    up(rig, at);
}

/// The track is the row less the value field; `fraction` runs along the track.
fn on_track(row: Bounds<Pixels>, fraction: f32) -> Point<Pixels> {
    let track = f32::from(row.size.width) - VALUE_FIELD - GAP;
    point(row.left() + px(track * fraction), row.center().y)
}

fn camera_bounds(rig: &mut Rig, c: CameraControl) -> Option<Bounds<Pixels>> {
    rig.app
        .cx
        .update(|cx| rig.camera.read(cx).control_bounds(c))
}

fn blur_bounds(rig: &mut Rig, c: BlurControl) -> Option<Bounds<Pixels>> {
    rig.app.cx.update(|cx| rig.blur.read(cx).control_bounds(c))
}

/// Scrubs past both ends and checks the drag reached only `get`'s field, clamped to `range`, as one live adjustment.
fn scrub_to_both_ends(
    rig: &mut Rig,
    name: &str,
    row: Bounds<Pixels>,
    range: std::ops::RangeInclusive<f64>,
    get: impl Fn(&BackgroundSettings) -> f64,
    others: impl Fn(&BackgroundSettings) -> Vec<f64>,
) {
    let before = settings(rig);
    let seen = record(rig);
    let start = on_track(row, 0.3);
    down(rig, start, 1);
    move_to(rig, point(row.right() + px(80.), start.y));
    assert_eq!(get(&settings(rig)), *range.end(), "{name} stops at its top");
    move_to(rig, point(row.left() - px(80.), start.y));
    assert_eq!(
        get(&settings(rig)),
        *range.start(),
        "{name} stops at its bottom"
    );
    up(rig, point(row.left() - px(80.), start.y));
    assert_eq!(
        others(&settings(rig)),
        others(&before),
        "{name} moved another setting"
    );
    let seen = seen.borrow();
    assert_eq!(seen.first(), Some(&LookEvent::LiveAdjust(true)), "{name}");
    assert_eq!(seen.last(), Some(&LookEvent::LiveAdjust(false)), "{name}");
    let inside = &seen[1..seen.len() - 1];
    assert!(
        !inside.is_empty() && inside.iter().all(|e| *e == LookEvent::Changed),
        "{name} changes stay inside one live adjustment: {seen:?}"
    );
}

#[test]
fn every_camera_slider_drives_its_own_field_within_its_range() {
    let mut rig = rig(ThemeMode::Dark, BackgroundSettings::default());
    let expected = [
        (CameraControl::TiltX, -45.0..=45.0),
        (CameraControl::TiltY, -45.0..=45.0),
        (CameraControl::Roll, -45.0..=45.0),
        (CameraControl::Fov, 18.0..=80.0),
        (CameraControl::Zoom, 0.4..=2.5),
        (CameraControl::PanX, -0.5..=0.5),
        (CameraControl::PanY, -0.5..=0.5),
        (CameraControl::RotateX, -60.0..=60.0),
        (CameraControl::RotateY, -60.0..=60.0),
    ];
    for (control, range) in expected {
        let row = camera_bounds(&mut rig, control).expect("the slider is drawn");
        scrub_to_both_ends(
            &mut rig,
            &format!("{control:?}"),
            row,
            range,
            move |s| control.get(s),
            move |s| {
                CameraControl::ALL
                    .into_iter()
                    .filter(|&c| c != control)
                    .map(|c| c.get(s))
                    .collect()
            },
        );
    }
}

#[test]
fn every_blur_slider_drives_its_own_field_within_its_range() {
    let mut look = BackgroundSettings::default();
    look.progressive_blur.is_enabled = true;
    look.progressive_blur.mode = BlurMode::Directional;
    let mut rig = rig(ThemeMode::Dark, look);
    let expected = [
        (BlurControl::Strength, 0.0..=40.0),
        (BlurControl::Falloff, 0.0..=1.0),
        (BlurControl::FocusSize, 0.0..=1.0),
        (BlurControl::Direction, 0.0..=180.0),
    ];
    for (control, range) in expected {
        let row = blur_bounds(&mut rig, control).expect("the slider is drawn");
        scrub_to_both_ends(
            &mut rig,
            &format!("{control:?}"),
            row,
            range,
            move |s| control.get(&s.progressive_blur),
            move |s| {
                let b = &s.progressive_blur;
                let mut rest: Vec<f64> = BlurControl::SLIDERS
                    .into_iter()
                    .filter(|&c| c != control)
                    .map(|c| c.get(b))
                    .collect();
                rest.extend([b.focus_x, b.focus_y]);
                rest
            },
        );
    }
}

#[test]
fn signed_sliders_land_exactly_on_zero_near_the_middle() {
    let mut look = BackgroundSettings::default();
    look.camera.tilt_x_degrees = 30.0;
    look.camera.pan_y = 0.3;
    let mut rig = rig(ThemeMode::Dark, look);
    for control in [CameraControl::TiltX, CameraControl::PanY] {
        let row = camera_bounds(&mut rig, control).unwrap();
        let zero = on_track(row, 0.5);
        click(&mut rig, zero + point(px(2.), px(0.)));
        assert_eq!(control.get(&settings(&mut rig)), 0.0, "{control:?} snaps");
        click(&mut rig, zero + point(px(12.), px(0.)));
        assert!(
            control.get(&settings(&mut rig)) > 0.0,
            "{control:?} leaves zero past the detent"
        );
    }
    let fov = camera_bounds(&mut rig, CameraControl::Fov).unwrap();
    click(&mut rig, on_track(fov, 0.0));
    assert_eq!(CameraControl::Fov.get(&settings(&mut rig)), 18.0);
}

#[test]
fn direction_shows_only_for_a_directional_blur() {
    let mut look = BackgroundSettings::default();
    look.progressive_blur.is_enabled = true;
    let mut rig = rig(ThemeMode::Light, look);
    assert!(blur_bounds(&mut rig, BlurControl::Direction).is_none());

    let mode = blur_bounds(&mut rig, BlurControl::Mode).unwrap();
    let directional = point(mode.left() + mode.size.width * 0.75, mode.center().y);
    click(&mut rig, directional);
    assert_eq!(
        settings(&mut rig).progressive_blur.mode,
        BlurMode::Directional
    );
    assert!(blur_bounds(&mut rig, BlurControl::Direction).is_some());

    let radial = point(mode.left() + mode.size.width * 0.25, mode.center().y);
    click(&mut rig, radial);
    assert_eq!(settings(&mut rig).progressive_blur.mode, BlurMode::Radial);
    assert!(blur_bounds(&mut rig, BlurControl::Direction).is_none());
}

#[test]
fn the_focus_pad_moves_the_focus_and_a_double_click_centres_it() {
    let mut look = BackgroundSettings::default();
    look.progressive_blur.is_enabled = true;
    let mut rig = rig(ThemeMode::Dark, look);
    let pad = blur_bounds(&mut rig, BlurControl::FocusPad).unwrap();
    let at = |fx: f32, fy: f32| {
        point(
            pad.left() + pad.size.width * fx,
            pad.top() + pad.size.height * fy,
        )
    };
    let focus = |rig: &mut Rig| {
        let b = settings(rig).progressive_blur;
        (b.focus_x, b.focus_y)
    };
    let near = |got: (f64, f64), want: (f64, f64)| {
        (got.0 - want.0).abs() < 0.02 && (got.1 - want.1).abs() < 0.02
    };

    let seen = record(&mut rig);
    down(&mut rig, at(0.25, 0.25), 1);
    assert!(near(focus(&mut rig), (0.25, 0.25)), "{:?}", focus(&mut rig));
    move_to(&mut rig, at(0.8, 0.6));
    assert!(near(focus(&mut rig), (0.8, 0.6)), "{:?}", focus(&mut rig));
    up(&mut rig, at(0.8, 0.6));
    {
        let seen = seen.borrow();
        assert_eq!(seen.first(), Some(&LookEvent::LiveAdjust(true)));
        assert_eq!(seen.last(), Some(&LookEvent::LiveAdjust(false)));
    }

    down(&mut rig, at(0.1, 0.9), 2);
    up(&mut rig, at(0.1, 0.9));
    assert_eq!(focus(&mut rig), (0.5, 0.5), "double-click centres");
}

#[test]
fn apply_to_changes_what_focus_size_says_it_measures() {
    let mut look = BackgroundSettings::default();
    look.progressive_blur.is_enabled = true;
    let mut rig = rig(ThemeMode::Dark, look);
    let help = |rig: &mut Rig| {
        rig.app
            .cx
            .update(|cx| rig.blur.read(cx).focus_size_help(cx))
    };
    assert_eq!(
        help(&mut rig).as_ref(),
        "Choose the size of the sharp area across the scene"
    );

    let apply_to = blur_bounds(&mut rig, BlurControl::ApplyTo).unwrap();
    let screenshot = point(
        apply_to.left() + apply_to.size.width * 0.25,
        apply_to.center().y,
    );
    click(&mut rig, screenshot);
    assert_eq!(
        settings(&mut rig).progressive_blur.edge_mode,
        BlurEdgeMode::Clipped
    );
    assert_eq!(
        help(&mut rig).as_ref(),
        "Choose the size of the sharp area inside the frame"
    );
}

#[test]
fn a_blur_switched_off_ignores_its_controls() {
    let mut rig = rig(ThemeMode::Dark, BackgroundSettings::default());
    let before = settings(&mut rig).progressive_blur;
    let strength = blur_bounds(&mut rig, BlurControl::Strength).unwrap();
    click(&mut rig, on_track(strength, 0.9));
    let pad = blur_bounds(&mut rig, BlurControl::FocusPad).unwrap();
    click(&mut rig, pad.origin + point(px(10.), px(10.)));
    assert_eq!(settings(&mut rig).progressive_blur, before);
}

fn shoot(mode: ThemeMode, name: &str, scrub: bool) {
    let mut look = BackgroundSettings::default();
    look.camera.tilt_y_degrees = 12.0;
    look.camera.zoom = 0.92;
    look.progressive_blur.is_enabled = true;
    look.progressive_blur.mode = match mode {
        ThemeMode::Light => BlurMode::Radial,
        _ => BlurMode::Directional,
    };
    look.progressive_blur.direction_degrees = 24.0;
    look.progressive_blur.focus_x = 0.62;
    look.progressive_blur.focus_y = 0.4;
    let mut rig = rig(mode, look);
    if scrub {
        let row = camera_bounds(&mut rig, CameraControl::RotateY).unwrap();
        down(&mut rig, on_track(row, 0.5), 1);
        move_to(&mut rig, on_track(row, 0.68));
        let slider: Entity<InspectorSlider> = rig
            .app
            .cx
            .update(|cx| rig.camera.read(cx).slider(CameraControl::RotateY))
            .unwrap();
        rig.app
            .cx
            .update(|cx| slider.update(cx, |s, cx| s.preview(Interaction::Pressed, cx)));
    }
    rig.app
        .cx
        .advance_clock(std::time::Duration::from_millis(400));
    rig.app.settle();
    rig.app.screenshot(name);
}

#[test]
fn screenshots_camera_and_blur() {
    shoot(ThemeMode::Dark, "inspector-effects-dark", false);
    shoot(ThemeMode::Light, "inspector-effects-light", false);
    shoot(ThemeMode::Dark, "inspector-effects-scrub", true);
}
