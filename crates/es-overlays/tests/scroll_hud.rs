mod window_mode_support;

use es_geom::{Point, Rect};
use es_overlays::capture::scroll_hud::{HUD, hud_origin, outline, preview_column};
use es_overlays::capture::{HudEvent, ScrollHud};
use window_mode_support::{Surface, bluish, rgb};

const W: f32 = HUD.0 as f32;
const H: f32 = HUD.1 as f32;
const CANCEL: (f32, f32) = (18., 18.);
const AUTO: (f32, f32) = (283., 18.);
const STOP: (f32, f32) = (364., 18.);

fn reddish(p: [u8; 3]) -> bool {
    i32::from(p[0]) > i32::from(p[2]) + 60
}

#[test]
fn the_title_counts_the_stitched_size_in_points() {
    let mut s = Surface::open(W, H, |_| ScrollHud::new(true));
    let view = s.view.clone();
    assert_eq!(
        s.app.cx.update(|cx| view.read(cx).title()),
        "Scroll Capture"
    );
    s.app
        .cx
        .update(|cx| view.update(cx, |hud, cx| hud.set_size(640, 1820, cx)));
    assert_eq!(
        s.app.cx.update(|cx| view.read(cx).title()),
        "Scroll Capture  ·  640×1820"
    );
    let shot = s.app.screenshot("scroll-hud-mid-capture");
    assert!(
        bluish(rgb(&shot, AUTO.0 - 38., AUTO.1)),
        "Auto Scroll is the accent"
    );
    assert!(reddish(rgb(&shot, STOP.0 - 22., STOP.1)), "Stop is red");
}

#[test]
fn the_buttons_say_what_they_do() {
    let mut s = Surface::open(W, H, |_| ScrollHud::new(true));
    let (events, _sub) = s.record::<HudEvent>();
    s.click(CANCEL.0, CANCEL.1);
    s.click(AUTO.0, AUTO.1);
    s.click(STOP.0, STOP.1);
    assert_eq!(
        *events.borrow(),
        vec![HudEvent::Cancel, HudEvent::Auto(true), HudEvent::Stop]
    );
    let view = s.view.clone();
    s.app
        .cx
        .update(|cx| view.update(cx, |hud, cx| hud.set_auto(true, cx)));
    s.app.screenshot("scroll-hud-scrolling");
    s.click(AUTO.0, AUTO.1);
    assert_eq!(
        events.borrow().last(),
        Some(&HudEvent::Auto(false)),
        "while scrolling, the same pill pauses it"
    );
}

#[test]
fn without_a_virtual_pointer_there_is_no_auto_scroll() {
    let mut s = Surface::open(W, H, |_| ScrollHud::new(false));
    let (events, _sub) = s.record::<HudEvent>();
    let view = s.view.clone();
    s.app
        .cx
        .update(|cx| view.update(cx, |hud, cx| hud.set_auto(true, cx)));
    assert_eq!(s.app.cx.update(|cx| view.read(cx).auto()), None);
    s.click(AUTO.0, AUTO.1);
    s.click(STOP.0, STOP.1);
    assert_eq!(*events.borrow(), vec![HudEvent::Stop]);
}

const OUTPUT: Rect = Rect::new(0., 0., 1280., 720.);

#[test]
fn the_bar_sits_under_the_area_or_above_it_and_stays_on_the_output() {
    let area = Rect::new(400., 100., 800., 500.);
    assert_eq!(hud_origin(area, OUTPUT), Point::new(400., 506.));
    let low = Rect::new(400., 300., 800., 700.);
    assert_eq!(
        hud_origin(low, OUTPUT),
        Point::new(400., 258.),
        "no room below"
    );
    let corner = Rect::new(0., 0., 300., 720.);
    let at = hud_origin(corner, OUTPUT);
    assert_eq!(at, Point::new(4., 4.), "clamped 4 points inside");
}

#[test]
fn the_preview_takes_the_side_with_room() {
    let area = Rect::new(400., 100., 800., 500.);
    assert_eq!(
        preview_column(area, OUTPUT),
        Some(Rect::new(812., 20., 1012., 500.))
    );
    let right = Rect::new(900., 100., 1200., 500.);
    assert_eq!(
        preview_column(right, OUTPUT),
        Some(Rect::new(688., 20., 888., 500.))
    );
    let wide = Rect::new(100., 100., 1180., 500.);
    assert_eq!(preview_column(wide, OUTPUT), None);
}

#[test]
fn the_outline_never_covers_the_area() {
    let area = Rect::new(400., 100., 800., 500.);
    let strips = outline(area, OUTPUT);
    assert_eq!(strips.len(), 4);
    for s in &strips {
        let overlap = s.intersect(area);
        assert!(
            overlap.width() <= 0. || overlap.height() <= 0.,
            "{s:?} overlaps the area"
        );
    }
    let full = outline(OUTPUT, OUTPUT);
    assert!(full.is_empty(), "the whole output leaves no room outside");
}
