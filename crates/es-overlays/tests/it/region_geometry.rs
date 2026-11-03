use es_geom::{Point, Rect, Size};
use es_model::platform::{Frame, OutputId, Transform};
use es_overlays::capture::geometry::{
    Handle, PixelRect, apply, counts, crop, drawn, handle_at, to_pixels,
};
use std::sync::Arc;
use std::time::Instant;

const SEL: Rect = Rect::new(100., 100., 300., 200.);
const SCREEN: Rect = Rect::new(0., 0., 1000., 800.);

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[test]
fn a_press_near_a_handle_picks_that_handle_inside_picks_move_outside_picks_nothing() {
    assert_eq!(
        handle_at(p(303., 197.), SEL, true),
        Some(Handle::BottomRight)
    );
    assert_eq!(handle_at(p(200., 101.), SEL, true), Some(Handle::Top));
    assert_eq!(handle_at(p(150., 150.), SEL, true), Some(Handle::Move));
    assert_eq!(handle_at(p(50., 50.), SEL, true), None);
}

#[test]
fn the_handle_zone_is_eight_points_and_ends_there() {
    assert_eq!(handle_at(p(308., 100.), SEL, true), Some(Handle::TopRight));
    assert_eq!(handle_at(p(308.5, 100.), SEL, true), None);
    assert_eq!(
        handle_at(p(100., 150. + 7.9), SEL, false),
        Some(Handle::Left)
    );
}

#[test]
fn a_preselected_area_resizes_from_its_handles_but_a_press_inside_draws_a_new_one() {
    assert_eq!(handle_at(p(150., 150.), SEL, false), None);
    assert_eq!(handle_at(p(100., 100.), SEL, false), Some(Handle::TopLeft));
}

#[test]
fn a_tiny_selection_stays_resizable_because_handles_beat_move() {
    let tiny = Rect::new(10., 10., 14., 14.);
    assert_eq!(handle_at(p(12., 12.), tiny, true), Some(Handle::Top));
}

#[test]
fn an_edge_handle_moves_only_its_own_edge() {
    let r = apply(Handle::Right, p(0., 0.), p(50., 999.), SEL, SCREEN);
    assert_eq!(r, Rect::new(100., 100., 350., 200.));
    let r = apply(Handle::BottomLeft, p(0., 0.), p(-10., 20.), SEL, SCREEN);
    assert_eq!(r, Rect::new(90., 100., 300., 220.));
}

#[test]
fn dragging_past_the_opposite_edge_flips_instead_of_going_negative() {
    let r = apply(Handle::Left, p(0., 0.), p(250., 0.), SEL, SCREEN);
    assert_eq!(r, Rect::new(300., 100., 350., 200.));
}

#[test]
fn resize_clamps_to_the_output_and_move_keeps_the_size_on_it() {
    let r = apply(Handle::TopLeft, p(0., 0.), p(-500., -500.), SEL, SCREEN);
    assert_eq!(r, Rect::new(0., 0., 300., 200.));
    let r = apply(Handle::Move, p(0., 0.), p(900., -10.), SEL, SCREEN);
    assert_eq!(r, Rect::new(800., 90., 1000., 190.));
}

#[test]
fn a_backwards_drag_is_normalised_and_clamped_to_the_output() {
    let r = drawn(p(300., 250.), p(-40., 120.), SCREEN);
    assert_eq!(r, Rect::new(0., 120., 300., 250.));
}

#[test]
fn only_a_rectangle_larger_than_three_by_three_counts() {
    assert!(!counts(Rect::new(0., 0., 3., 50.)));
    assert!(!counts(Rect::new(0., 0., 50., 3.)));
    assert!(counts(Rect::new(0., 0., 3.5, 3.5)));
}

const OUTPUT: Size = Size::new(1280., 720.);
const PHYS: (u32, u32) = (1920, 1080);

#[test]
fn edges_are_rounded_so_a_fractional_scale_keeps_the_right_edge() {
    let sel = Rect::new(101.6, 50., 303.2, 150.);
    let got = to_pixels(sel, OUTPUT, PHYS).expect("non-empty");
    assert_eq!(
        got,
        PixelRect {
            x: 152,
            y: 75,
            w: 303,
            h: 150
        }
    );
    let separately = (sel.width() * 1.5).round() as u32;
    assert_eq!(
        separately, 302,
        "the case this guards: rounding the size alone loses the right edge's pixel"
    );
    assert_eq!(got.x + got.w, (sel.x1 * 1.5).round() as u32);
}

#[test]
fn neighbouring_selections_tile_the_frame_without_a_gap_or_overlap() {
    let (a, b, c) = (33.4, 101.6, 303.2);
    let left = to_pixels(Rect::new(a, 0., b, 10.), OUTPUT, PHYS).expect("left");
    let right = to_pixels(Rect::new(b, 0., c, 10.), OUTPUT, PHYS).expect("right");
    let both = to_pixels(Rect::new(a, 0., c, 10.), OUTPUT, PHYS).expect("both");
    assert_eq!(left.x + left.w, right.x);
    assert_eq!(left.w + right.w, both.w);
}

#[test]
fn the_scale_comes_from_the_frame_not_a_nominal_factor() {
    let got = to_pixels(
        Rect::new(0., 0., 100., 100.),
        Size::new(1000., 500.),
        (2000, 1000),
    );
    assert_eq!(got.map(|g| (g.w, g.h)), Some((200, 200)));
    assert_eq!(to_pixels(Rect::new(5., 5., 5.1, 9.), OUTPUT, PHYS), None);
}

fn frame() -> Frame {
    let (w, h) = (30u32, 20u32);
    let mut bgra = Vec::new();
    for y in 0..h {
        for x in 0..w {
            bgra.extend_from_slice(&[x as u8, y as u8, 7, 255]);
        }
    }
    Frame {
        output: Some(OutputId("HEADLESS-1".into())),
        size_px: (w, h),
        logical: Rect::new(2000., 100., 2020., 113.333_333_333_333_33),
        scale: 1.5,
        bgra: Arc::from(bgra),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    }
}

#[test]
fn a_crop_takes_exactly_the_selected_pixels_and_keeps_its_place_on_the_desktop() {
    let f = frame();
    let local = Rect::new(2., 4., 6., 8.);
    let out = crop(&f, local).expect("crop");
    assert_eq!(out.size_px, (6, 6));
    assert_eq!(out.bgra.len(), 6 * 6 * 4);
    assert_eq!(&out.bgra[..4], &[3, 6, 7, 255], "top-left is pixel (3, 6)");
    let last = out.bgra.len() - 4;
    assert_eq!(
        &out.bgra[last..],
        &[8, 11, 7, 255],
        "bottom-right is pixel (8, 11)"
    );
    assert_eq!(out.logical, Rect::new(2002., 104., 2006., 108.));
    assert_eq!(out.output, f.output);
}

#[test]
fn a_crop_past_the_frame_is_clipped_to_it_and_an_empty_one_is_refused() {
    let f = frame();
    let out = crop(&f, Rect::new(16., 10., 40., 40.)).expect("crop");
    assert_eq!(out.size_px, (6, 5));
    assert!(crop(&f, Rect::new(25., 25., 40., 40.)).is_none());
}
