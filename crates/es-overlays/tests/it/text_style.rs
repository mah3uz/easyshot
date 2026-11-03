use crate::window_mode_support;

use es_geom::Rect;
use es_overlays::capture::{RegionSelector, SelectorEvent, SelectorStyle};
use window_mode_support::{Surface, bluish, frame, near, rgb};

const W: f32 = 640.;
const H: f32 = 400.;

fn light(_: u32, y: u32) -> [u8; 3] {
    if (y / 60).is_multiple_of(2) {
        [250, 250, 250]
    } else {
        [235, 235, 238]
    }
}

#[test]
fn text_style_is_undimmed_dashed_and_tinted_captures_on_release_and_ignores_space() {
    let frozen = frame(W, H, 2., light);
    let mut s = Surface::open(W, H, |cx| {
        RegionSelector::new(frozen, None, true, cx).with_style(SelectorStyle::Text)
    });
    let (events, _sub) = s.record::<SelectorEvent>();
    s.key("space");
    s.hover(100., 100., false);
    s.down(100., 100.);
    s.hover(200., 150., true);
    s.hover(300., 200., true);
    let shot = s.app.screenshot("text-style");
    assert!(
        near(rgb(&shot, 50., 50.), light(100, 100), 1),
        "outside is the frame, not dimmed"
    );
    let inside = rgb(&shot, 200., 150.);
    assert!(
        inside[2] > inside[0] + 8 && inside[0] < 240,
        "inside carries a light accent tint: {inside:?}"
    );
    let edge: Vec<bool> = (0..300)
        .map(|i| bluish(rgb(&shot, 110. + i as f32 * 0.5, 100.25)))
        .collect();
    let (on, off) = (
        edge.iter().filter(|b| **b).count(),
        edge.iter().filter(|b| !**b).count(),
    );
    assert!(
        on > 40 && off > 40,
        "the top edge is dashed: {on} on, {off} off"
    );
    s.up(300., 200.);
    assert_eq!(
        *events.borrow(),
        vec![
            SelectorEvent::Began,
            SelectorEvent::Capture(Rect::new(100., 100., 300., 200.))
        ],
        "no window mode, and the area is taken on release"
    );
}
