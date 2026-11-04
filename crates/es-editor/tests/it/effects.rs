#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::interaction_support;

use canvas_support::*;
use es_anno::{BuildContext, build_draw_list};
use es_editor::EditorSession;
use es_editor::canvas::effects::{EffectsState, SETTLE_DELAY, Showing};
use es_model::{BackgroundColor, BackgroundSettings, BackgroundStyle, CameraSettings, Tool};
use es_ui::motion::{COMPOSITE_FADE, set_reduce_motion};
use es_ui::tokens::ThemeMode;
use image::RgbaImage;
use interaction_support::*;
use std::time::Duration;
use tiny_skia::{IntSize, Pixmap};

fn stage() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Solid(BackgroundColor::custom(0.25, 0.45, 0.85)),
        ..BackgroundSettings::default()
    }
}

fn tilted(degrees: f64) -> BackgroundSettings {
    let mut s = stage();
    s.camera = CameraSettings {
        tilt_x_degrees: degrees,
        tilt_y_degrees: -8.0,
        ..CameraSettings::default()
    };
    s
}

fn blurred(focus: f64) -> BackgroundSettings {
    let mut s = stage();
    s.progressive_blur.is_enabled = true;
    s.progressive_blur.focus_size = focus;
    s
}

fn state(app: &App) -> EffectsState {
    canvas(app).read_with(&app.cx, |c, _| c.effects())
}

fn set_look(app: &mut App, look: BackgroundSettings) {
    let c = canvas(app);
    app.cx
        .update(|cx| c.update(cx, |c, cx| c.set_background(look, cx)));
    app.settle();
}

fn live(app: &mut App, on: bool) {
    let c = canvas(app);
    app.cx.update(|cx| {
        c.update(cx, |c, cx| {
            if on {
                c.begin_live_adjust(cx)
            } else {
                c.end_live_adjust(cx)
            }
        })
    });
    app.settle();
}

fn wait(app: &mut App, d: Duration) {
    app.cx.advance_clock(d);
    app.settle();
}

fn lab(p: [u8; 4]) -> [f64; 3] {
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(p[0]), lin(p[1]), lin(p[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

fn delta_e(a: [u8; 4], b: [u8; 4]) -> f64 {
    let (p, q) = (lab(a), lab(b));
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
}

/// The share of the canvas's pixels that differ from the export by more than dE 3, at 100 %.
fn off_the_export(
    app: &mut App,
    session: &EditorSession,
    look: &BackgroundSettings,
    (w, h): (u32, u32),
    name: &str,
) -> f64 {
    let shot = app.screenshot(&format!("{name}-canvas"));
    let origin = app
        .cx
        .update(|cx| session.viewport.read(cx).viewport().origin);
    let lay = es_render::beautify::layout(f64::from(w), f64::from(h), look).pixel_aligned();
    let ox = (origin.x * 2.0 - lay.image.x0).round() as u32;
    let oy = (origin.y * 2.0 - lay.image.y0).round() as u32;

    let list = session.doc.read_with(&app.cx, |d, _| {
        build_draw_list(d.editor().doc(), &BuildContext::new(&**TEXT))
    });
    let image = Pixmap::from_vec(base(w, h).into_raw(), IntSize::from_wh(w, h).unwrap()).unwrap();
    let export = es_render::beautify::compose(&image, &list, look).unwrap();
    let (ew, eh) = (export.width(), export.height());
    assert!(
        ox + ew <= shot.width() && oy + eh <= shot.height(),
        "the canvas fits the window"
    );

    let mut diff = RgbaImage::new(ew, eh);
    let mut off = 0usize;
    for y in 0..eh {
        for x in 0..ew {
            let c = shot.get_pixel(ox + x, oy + y).0;
            let p = export.pixel(x, y).unwrap().demultiply();
            let e = [p.red(), p.green(), p.blue(), p.alpha()];
            if delta_e(c, e) > 3.0 {
                off += 1;
                diff.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            } else {
                diff.put_pixel(x, y, image::Rgba([c[0] / 3, c[1] / 3, c[2] / 3, 255]));
            }
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    diff.save(dir.join(format!("{name}-diff.png"))).unwrap();
    let share = off as f64 / f64::from(ew * eh);
    eprintln!(
        "{name}: {:.3} % of pixels differ by more than dE 3",
        share * 100.0
    );
    share
}

fn at_100_percent(look: &BackgroundSettings, size: (u32, u32)) -> (App, EditorSession) {
    let (mut app, session) = open(
        800.0,
        520.0,
        opened(size.0, size.1, every_kind(false, true), look.clone()),
    );
    app.cx.update(|cx| {
        session
            .viewport
            .update(cx, |v, cx| v.set_percent(100.0, cx))
    });
    app.settle();
    (app, session)
}

#[test]
fn the_settled_composite_matches_the_export_at_100_percent() {
    let size = (1200u32, 640u32);
    let mut look = tilted(14.0);
    look.progressive_blur.is_enabled = true;
    let (mut app, session) = at_100_percent(&look, size);
    wait(&mut app, SETTLE_DELAY);
    assert_eq!(state(&app).showing, Some(Showing::Settled));
    let share = off_the_export(&mut app, &session, &look, size, "effects-parity");
    assert!(share <= 0.02, "{:.3} % of pixels differ", share * 100.0);
}

#[test]
fn a_live_camera_frame_matches_the_export_at_100_percent() {
    let size = (1200u32, 640u32);
    for blur in [false, true] {
        let mut look = tilted(6.0);
        look.progressive_blur.is_enabled = blur;
        let (mut app, session) = at_100_percent(&look, size);
        wait(&mut app, SETTLE_DELAY);
        live(&mut app, true);
        for degrees in [8.0, 11.0, 14.0] {
            look = tilted(degrees);
            look.progressive_blur.is_enabled = blur;
            set_look(&mut app, look.clone());
        }
        assert_eq!(state(&app).showing, Some(Showing::Live));
        let name = if blur {
            "effects-live-blur"
        } else {
            "effects-live"
        };
        let share = off_the_export(&mut app, &session, &look, size, name);
        assert!(
            share <= 0.02,
            "{name}: {:.3} % of pixels differ",
            share * 100.0
        );
    }
}

/// Edge energy: the summed luminance steps between neighbouring pixels.
fn sharpness(shot: &RgbaImage) -> f64 {
    let luma = |x: u32, y: u32| {
        let p = shot.get_pixel(x, y).0;
        0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
    };
    let mut sum = 0.0;
    for y in 0..shot.height() - 1 {
        for x in 0..shot.width() - 1 {
            sum += (luma(x + 1, y) - luma(x, y)).abs() + (luma(x, y + 1) - luma(x, y)).abs();
        }
    }
    sum
}

#[test]
fn dragging_a_camera_slider_keeps_the_preview_as_sharp_as_the_settled_composite() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(true, true), tilted(4.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    live(&mut app, true);
    for degrees in [6.0, 9.0, 12.0] {
        set_look(&mut app, tilted(degrees));
    }
    let during = state(&app);
    assert_eq!(during.showing, Some(Showing::Live));
    let live_shot = app.screenshot("effects-sharp-live");
    live(&mut app, false);
    wait(&mut app, SETTLE_DELAY + COMPOSITE_FADE.duration);
    redraw(&mut app);
    let after = state(&app);
    assert_eq!(
        (after.showing, after.fading),
        (Some(Showing::Settled), false)
    );
    let settled_shot = app.screenshot("effects-sharp-settled");
    let ratio = sharpness(&live_shot) / sharpness(&settled_shot);
    assert!(
        (ratio - 1.0).abs() <= 0.01,
        "the live frame's edges are {ratio:.3} of the settled composite's"
    );
    assert_eq!(
        during.pixels, after.pixels,
        "the live frame has the settled composite's resolution"
    );
}

#[test]
fn a_live_frame_is_drawn_off_the_ui_thread_and_the_last_finished_one_stays_up() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), tilted(4.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    live(&mut app, true);
    set_look(&mut app, tilted(6.0));
    let before = state(&app);
    assert_eq!(
        (before.showing, before.live_frames),
        (Some(Showing::Live), 1)
    );
    let c = canvas(&app);
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            c.update(cx, |c, cx| c.set_background(tilted(9.0), cx));
            window.draw(cx).clear(cx);
        })
        .expect("window is open");
    let s = state(&app);
    assert_eq!(
        (s.showing, s.live_frames, s.released),
        (Some(Showing::Live), 1, before.released),
        "the change is drawn off the UI thread; the frame before it stays painted"
    );
    app.settle();
    let s = state(&app);
    assert_eq!(
        (s.showing, s.live_frames),
        (Some(Showing::Live), 2),
        "the frame lands and replaces the one before"
    );
}

#[test]
fn the_settled_composite_fades_in_over_the_live_frame_unless_reduce_motion_is_on() {
    for reduce in [false, true] {
        let (mut app, _session) = open(
            900.0,
            560.0,
            opened(1200, 640, every_kind(false, false), blurred(0.6)),
        );
        app.cx.update(|cx| set_reduce_motion(reduce, cx));
        wait(&mut app, SETTLE_DELAY);
        live(&mut app, true);
        set_look(&mut app, blurred(0.4));
        live(&mut app, false);
        wait(&mut app, SETTLE_DELAY);
        let s = state(&app);
        assert_eq!(s.showing, Some(Showing::Settled), "reduce {reduce}");
        if reduce {
            assert_eq!(
                (s.fading, s.held),
                (false, 1),
                "Reduce Motion swaps at once"
            );
            continue;
        }
        assert_eq!(
            (s.fading, s.held),
            (true, 2),
            "the live frame stays under the composite while it fades in"
        );
        wait(&mut app, COMPOSITE_FADE.duration);
        redraw(&mut app);
        let s = state(&app);
        assert_eq!(
            (s.fading, s.held),
            (false, 1),
            "the outgoing frame is released when the fade ends"
        );
    }
}

#[test]
fn a_burst_of_changes_renders_one_composite_and_the_old_one_stays_up_meanwhile() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), tilted(10.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    let first = state(&app);
    assert_eq!(
        (first.composites_rendered, first.showing),
        (1, Some(Showing::Settled))
    );

    for step in 1..=5 {
        set_look(&mut app, tilted(10.0 + 2.0 * f64::from(step)));
        wait(&mut app, Duration::from_millis(40));
        let s = state(&app);
        assert_eq!(
            s.showing,
            Some(Showing::Settled),
            "step {step}: the old composite is still painted"
        );
        assert_eq!(
            s.composites_rendered, 1,
            "step {step}: nothing renders inside the quiet time"
        );
        assert!(s.pending, "step {step}");
    }
    wait(&mut app, SETTLE_DELAY);
    let s = state(&app);
    assert_eq!(s.composites_rendered, 2, "five changes, one composite");
    assert_eq!(
        (s.held, s.fading, s.released),
        (2, true, 0),
        "the old composite stays under the new one while it fades in"
    );
    wait(&mut app, COMPOSITE_FADE.duration);
    redraw(&mut app);
    let s = state(&app);
    assert_eq!((s.held, s.released, s.pending), (1, 1, false));
}

#[test]
fn with_the_camera_and_blur_off_nothing_extra_is_rendered_or_painted() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(true, true), stage()),
    );
    wait(&mut app, Duration::from_secs(1));
    redraw(&mut app);
    assert_eq!(state(&app), EffectsState::default());
    let frame = canvas(&app).read_with(&app.cx, |c, _| c.last_frame());
    assert!(
        frame.tiles_painted > 0 && frame.paths_painted > 0,
        "the flat canvas paints: {frame:?}"
    );
}

#[test]
fn every_replaced_image_is_released_and_turning_the_effects_off_frees_the_last() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), tilted(10.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    live(&mut app, true);
    for step in 1..=8 {
        set_look(&mut app, tilted(10.0 + f64::from(step)));
        let s = state(&app);
        assert_eq!(s.showing, Some(Showing::Live), "step {step}");
        assert_eq!(s.held, 1, "step {step}: one image on hand");
    }
    let s = state(&app);
    assert_eq!(
        (s.live_frames, s.released),
        (8, 8),
        "each live frame replaced the one before"
    );
    live(&mut app, false);
    assert_eq!(
        state(&app).showing,
        Some(Showing::Live),
        "the last live frame stays until the composite lands"
    );
    wait(&mut app, SETTLE_DELAY);
    assert_eq!(state(&app).showing, Some(Showing::Settled));

    set_look(&mut app, stage());
    redraw(&mut app);
    let s = state(&app);
    assert_eq!(
        (s.showing, s.held, s.released, s.pending),
        (None, 0, 10, false)
    );
}

#[test]
fn editing_an_annotation_shows_the_flat_canvas_and_the_composite_returns_after() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), tilted(12.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    assert_eq!(state(&app).showing, Some(Showing::Settled));
    set_tool(&mut app, &session, Tool::Select);
    let id = ids(&app, &session)[0];
    select(&mut app, &session, vec![id]);
    assert_eq!(
        state(&app).showing,
        None,
        "a selection is edited on the flat canvas"
    );
    select(&mut app, &session, Vec::new());
    assert_eq!(
        state(&app).showing,
        Some(Showing::Settled),
        "nothing changed, so the composite is still good"
    );
    assert_eq!(state(&app).composites_rendered, 1);
}

#[test]
fn live_and_settled_pictures_in_both_themes() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        for (kind, from, to) in [
            ("camera", tilted(4.0), tilted(18.0)),
            ("blur", blurred(0.6), blurred(0.3)),
        ] {
            let (mut app, _session) = open(
                900.0,
                560.0,
                opened(1200, 640, every_kind(true, true), from),
            );
            app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
            wait(&mut app, SETTLE_DELAY);
            live(&mut app, true);
            set_look(&mut app, to);
            assert_eq!(state(&app).showing, Some(Showing::Live));
            app.screenshot(&format!("effects-{kind}-live-{name}"));
            live(&mut app, false);
            wait(&mut app, SETTLE_DELAY);
            assert_eq!(state(&app).showing, Some(Showing::Settled));
            app.screenshot(&format!("effects-{kind}-settled-{name}"));
        }
    }
}

#[test]
fn the_first_frame_of_a_drag_keeps_the_settled_image_while_the_stage_builds_off_thread() {
    let (mut app, _session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), tilted(10.0)),
    );
    wait(&mut app, SETTLE_DELAY);
    let c = canvas(&app);
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            c.update(cx, |c, cx| {
                c.begin_live_adjust(cx);
                c.set_background(tilted(14.0), cx);
            });
            window.draw(cx).clear(cx);
        })
        .expect("window is open");
    let s = state(&app);
    assert_eq!(
        (s.showing, s.live_frames),
        (Some(Showing::Settled), 0),
        "the frame that starts a drag must not build the stage on the UI thread"
    );
    app.settle();
    let s = state(&app);
    assert_eq!(
        (s.showing, s.live_frames),
        (Some(Showing::Live), 1),
        "the stage lands and the next frame draws from it"
    );
}

fn marked() -> BackgroundSettings {
    let mut s = stage();
    s.watermark.is_enabled = true;
    s.watermark.text = "easyshot".into();
    s.watermark.opacity = 0.6;
    s
}

#[test]
fn a_watermark_alone_draws_over_the_flat_canvas_and_stays_while_editing() {
    let (mut app, session) = open(
        900.0,
        560.0,
        opened(1200, 640, every_kind(false, false), stage()),
    );
    wait(&mut app, SETTLE_DELAY);
    let plain = app.screenshot("watermark-off");
    set_look(&mut app, marked());
    wait(&mut app, SETTLE_DELAY);
    let s = state(&app);
    assert_eq!(
        (s.showing, s.composites_rendered, s.watermark),
        (None, 0, true),
        "a watermark needs no composite: {s:?}"
    );
    let shot = app.screenshot("watermark-on");
    let changed = plain
        .pixels()
        .zip(shot.pixels())
        .filter(|(a, b)| delta_e(a.0, b.0) > 3.0)
        .count();
    assert!(changed > 1000, "the watermark is painted: {changed} pixels");

    set_tool(&mut app, &session, Tool::Select);
    let id = ids(&app, &session)[0];
    select(&mut app, &session, vec![id]);
    assert!(state(&app).watermark, "the watermark stays while editing");

    set_look(&mut app, stage());
    redraw(&mut app);
    assert_eq!(state(&app), EffectsState::default());
}
