use crate::window_mode_support;

use es_geom::Rgba;
use es_overlays::capture::color::{LOUPE, hex, pixel_at, rgba};
use es_overlays::capture::{ColorEvent, ColorPicker};
use window_mode_support::{Surface, frame, near, rgb};

const W: f32 = 640.;
const H: f32 = 400.;

// Every frame pixel near the test points has its own colour.
fn unique(x: u32, y: u32) -> [u8; 3] {
    [
        ((x * 37) % 256) as u8,
        ((y * 53) % 256) as u8,
        (((x + y) * 17) % 256) as u8,
    ]
}

#[test]
fn hex_is_uppercase_rrggbb() {
    assert_eq!(hex(Rgba::new(1.0, 0.5, 0.0, 1.0)), "#FF8000");
    assert_eq!(hex(rgba([0x7c, 0x3a, 0xed])), "#7C3AED");
    assert_eq!(hex(rgba([0, 0, 0])), "#000000");
}

fn picks_the_pixel_under_the_pointer(scale: f64, name: &str) {
    let frozen = frame(W, H, scale, unique);
    let sampled = frozen.clone();
    let mut s = Surface::open(W, H, |cx| ColorPicker::new(frozen, cx));
    let (events, _sub) = s.record::<ColorEvent>();
    let (x, y) = (200.4_f32, 150.7_f32);
    s.hover(x, y, false);
    let shot = s.app.screenshot(name);
    let (px, py) = (
        (f64::from(x) * scale).floor() as u32,
        (f64::from(y) * scale).floor() as u32,
    );
    assert_eq!(
        pixel_at(&sampled, es_geom::Point::new(f64::from(x), f64::from(y))),
        Some((i64::from(px), i64::from(py)))
    );
    let cell = (LOUPE / 9.) as f32;
    assert!(
        near(rgb(&shot, x, y), unique(px, py), 1),
        "the loupe's centre cell shows the pixel under the pointer: {:?} vs {:?}",
        rgb(&shot, x, y),
        unique(px, py)
    );
    assert!(
        near(rgb(&shot, x + cell, y - cell), unique(px + 1, py - 1), 1),
        "the cell up and to the right shows that neighbour"
    );
    s.down(x, y);
    s.key("escape");
    let want = rgba(unique(px, py));
    assert_eq!(
        *events.borrow(),
        vec![ColorEvent::Picked(want), ColorEvent::Cancel]
    );
    let [r, g, b] = unique(px, py);
    assert_eq!(hex(want), format!("#{r:02X}{g:02X}{b:02X}"));
}

#[test]
fn picks_the_exact_pixel_at_scale_1() {
    picks_the_pixel_under_the_pointer(1.0, "color-loupe-1x");
}

#[test]
fn picks_the_exact_pixel_at_scale_1_5() {
    picks_the_pixel_under_the_pointer(1.5, "color-loupe-1.5x");
}

#[test]
fn arrows_move_the_sample_by_a_frame_pixel_and_enter_picks() {
    let scale = 1.5;
    let frozen = frame(W, H, scale, unique);
    let mut s = Surface::open(W, H, |cx| ColorPicker::new(frozen, cx));
    let (events, _sub) = s.record::<ColorEvent>();
    s.key("enter");
    assert!(
        events.borrow().is_empty(),
        "Enter picks nothing before there is a sample"
    );
    let (x, y) = (200.4_f32, 150.7_f32);
    s.hover(x, y, false);
    let (px, py) = (
        (f64::from(x) * scale).floor() as u32,
        (f64::from(y) * scale).floor() as u32,
    );
    s.key("right");
    s.key("enter");
    s.key("shift-down");
    s.key("left");
    s.key("space");
    s.key("escape");
    assert_eq!(
        *events.borrow(),
        vec![
            ColorEvent::Picked(rgba(unique(px + 1, py))),
            ColorEvent::Picked(rgba(unique(px, py + 10))),
            ColorEvent::Cancel,
        ],
        "Right moves one frame pixel, Shift+Down ten, Enter and Space pick, Esc cancels"
    );
}

#[test]
fn arrows_start_from_the_middle_and_stop_at_the_edge() {
    let frozen = frame(W, H, 1.0, unique);
    let mut s = Surface::open(W, H, |cx| ColorPicker::new(frozen, cx));
    let (events, _sub) = s.record::<ColorEvent>();
    s.key("down");
    s.key("enter");
    for _ in 0..40 {
        s.key("shift-right");
    }
    s.key("enter");
    let (mx, my) = ((W / 2.) as u32, (H / 2.) as u32);
    assert_eq!(
        *events.borrow(),
        vec![
            ColorEvent::Picked(rgba(unique(mx, my + 1))),
            ColorEvent::Picked(rgba(unique(W as u32 - 1, my + 1))),
        ]
    );
}
