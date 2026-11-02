#![allow(clippy::unwrap_used)]

use es_geom::Rect;
use es_model::{BackgroundSettings, CameraSettings};
use es_render::beautify::camera::{Homography, Quality, project, projection, warp};
use es_render::beautify::{Layout, layout};
use tiny_skia::{Color, Pixmap};

fn settings(camera: CameraSettings) -> BackgroundSettings {
    BackgroundSettings {
        padding: 0.35,
        camera,
        ..BackgroundSettings::default()
    }
}

fn card_layer(lay: &Layout) -> Pixmap {
    let (w, h) = lay.canvas_pixels();
    let mut layer = Pixmap::new(w, h).unwrap();
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(Color::from_rgba8(200, 40, 40, 255));
    let r = lay.card;
    let rect =
        tiny_skia::Rect::from_ltrb(r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32).unwrap();
    layer.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
    layer
}

fn alpha(p: &Pixmap, x: u32, y: u32) -> u8 {
    p.pixel(x, y).unwrap().alpha()
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5
}

#[test]
fn a_camera_that_does_not_move_the_card_returns_the_layer_untouched() {
    let s = settings(CameraSettings::default());
    let lay = layout(400.0, 300.0, &s);
    let layer = card_layer(&lay);
    let before = layer.data().to_vec();
    let out = project(layer, &lay, &s.camera, Quality::Full);
    assert_eq!(out.data(), &before[..]);
}

#[test]
fn identity_homography_warp_reproduces_the_layer() {
    let s = settings(CameraSettings::default());
    let lay = layout(400.0, 300.0, &s);
    let layer = card_layer(&lay);
    let out = warp(&layer, &Homography::IDENTITY, Quality::Full).unwrap();
    assert_eq!(out.data(), layer.data());
}

#[test]
fn tilt_moves_card_corners_to_the_perspective_positions() {
    let camera = CameraSettings {
        tilt_x_degrees: 20.0,
        ..CameraSettings::default()
    };
    let s = settings(camera.clone());
    let lay = layout(400.0, 300.0, &s);
    let canvas = lay.canvas_pixels();
    let p = projection(
        lay.card,
        lay.image,
        (f64::from(canvas.0), f64::from(canvas.1)),
        &camera,
    )
    .unwrap();

    let focal = 400.0 * (0.35 + 0.5 / 12f64.to_radians().tan());
    let theta = 20f64.to_radians();
    let c = lay.image.center();
    let expect = |sx: f64, sy: f64| {
        let (dx, dy) = (sx * 200.0, sy * 150.0);
        let k = focal / (focal - dx * theta.sin());
        (c.x + dx * theta.cos() * k, c.y + dy * k)
    };
    let want = [
        expect(-1.0, -1.0),
        expect(1.0, -1.0),
        expect(1.0, 1.0),
        expect(-1.0, 1.0),
    ];
    for (got, want) in p.quad.iter().zip(want) {
        assert!(near(*got, want), "{got:?} vs {want:?}");
    }
    assert!(
        p.quad.iter().all(|q| q.0 >= 0.0
            && q.1 >= 0.0
            && q.0 <= f64::from(canvas.0)
            && q.1 <= f64::from(canvas.1)),
        "the tilted card stays inside the canvas"
    );
    let corners = [
        (lay.card.x0, lay.card.y0),
        (lay.card.x1, lay.card.y0),
        (lay.card.x1, lay.card.y1),
        (lay.card.x0, lay.card.y1),
    ];
    for (i, corner) in corners.into_iter().enumerate() {
        let mapped = p.forward.apply(corner.0, corner.1).unwrap();
        assert!(near(mapped, p.quad[i]));
        let back = p.inverse.apply(mapped.0, mapped.1).unwrap();
        assert!(near(back, corner));
    }
}

#[test]
fn the_near_edge_of_a_tilted_card_is_taller_than_the_far_edge() {
    let camera = CameraSettings {
        tilt_x_degrees: 25.0,
        ..CameraSettings::default()
    };
    let lay = layout(400.0, 300.0, &settings(camera.clone()));
    let p = projection(lay.card, lay.image, (800.0, 600.0), &camera).unwrap();
    let left = p.quad[3].1 - p.quad[0].1;
    let right = p.quad[2].1 - p.quad[1].1;
    assert!(right > left, "positive x tilt brings the right edge closer");
}

#[test]
fn roll_rotates_the_corners_about_the_centre() {
    let camera = CameraSettings {
        roll_degrees: 90.0,
        ..CameraSettings::default()
    };
    let card = Rect::new(100.0, 100.0, 300.0, 300.0);
    let p = projection(card, card, (400.0, 400.0), &camera).unwrap();
    assert!(near(p.quad[0], (300.0, 100.0)), "{:?}", p.quad[0]);
    assert!(near(p.quad[1], (300.0, 300.0)), "{:?}", p.quad[1]);
}

#[test]
fn warp_keeps_alpha_zero_outside_the_projected_quad() {
    let camera = CameraSettings {
        tilt_x_degrees: 30.0,
        roll_degrees: 10.0,
        ..CameraSettings::default()
    };
    let s = settings(camera.clone());
    let lay = layout(400.0, 300.0, &s);
    let out = project(card_layer(&lay), &lay, &camera, Quality::Full);
    let (w, h) = (out.width(), out.height());
    let c = lay.image.center();
    assert_eq!(alpha(&out, c.x as u32, c.y as u32), 255);
    for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        assert_eq!(alpha(&out, x, y), 0);
    }
    let p = projection(lay.card, lay.image, (f64::from(w), f64::from(h)), &camera).unwrap();
    let min_x = p.quad.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
    assert_eq!(alpha(&out, (min_x - 3.0) as u32, c.y as u32), 0);
}

#[test]
fn half_quality_is_half_size_and_agrees_with_the_full_render() {
    let camera = CameraSettings {
        tilt_x_degrees: 20.0,
        tilt_y_degrees: -10.0,
        ..CameraSettings::default()
    };
    let s = settings(camera.clone());
    let lay = layout(400.0, 300.0, &s);
    let full = project(card_layer(&lay), &lay, &camera, Quality::Full);
    let half = project(card_layer(&lay), &lay, &camera, Quality::Half);
    assert_eq!(
        (half.width(), half.height()),
        (full.width().div_ceil(2), full.height().div_ceil(2))
    );
    let mut wrong = 0;
    for y in 0..half.height() {
        for x in 0..half.width() {
            let a = i32::from(alpha(&half, x, y));
            let b = i32::from(alpha(&full, x * 2, y * 2));
            if (a - b).abs() > 128 {
                wrong += 1;
            }
        }
    }
    let edge_pixels = 2 * (half.width() + half.height()) * 2;
    assert!(
        wrong < edge_pixels,
        "{wrong} pixels differ beyond the quad edge"
    );
}

#[test]
#[ignore = "timing"]
fn time_full_quality_warp_at_2560x1600() {
    let camera = CameraSettings {
        tilt_x_degrees: 20.0,
        tilt_y_degrees: 10.0,
        ..CameraSettings::default()
    };
    let s = BackgroundSettings {
        padding: 0.3,
        camera: camera.clone(),
        ..BackgroundSettings::default()
    };
    let lay = layout(1600.0, 1000.0, &s);
    let layer = card_layer(&lay);
    let (w, h) = lay.canvas_pixels();
    let start = std::time::Instant::now();
    let _ = project(layer, &lay, &camera, Quality::Full);
    eprintln!("camera warp {w}x{h}: {:?}", start.elapsed());
}
