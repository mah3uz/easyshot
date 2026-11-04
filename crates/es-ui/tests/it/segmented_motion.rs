use crate::support::{self, App, WIDTH};
use es_ui::{
    components::{Segment, Segmented},
    motion::{STEP_CHANGE, set_reduce_motion},
};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

struct Host {
    selected: usize,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            Segmented::new(
                "tabs",
                "Tabs",
                ["One", "Two", "Three", "Four"].map(Segment::text),
            )
            .selected(self.selected),
        )
    }
}

const ROW: u32 = 4;

fn select(app: &mut App, host: &Entity<Host>, index: usize) {
    app.cx.update(|cx| {
        host.update(cx, |h, cx| {
            h.selected = index;
            cx.notify();
        })
    });
    app.settle();
}

/// Mean x, in window pixels, of the pixels on a row above the labels that carry the thumb's fill.
fn thumb_x(app: &mut App, name: &str, fill: image::Rgba<u8>) -> f32 {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .unwrap_or_default();
    let image = app.screenshot(name);
    let xs: Vec<u32> = (0..image.width())
        .filter(|&x| *image.get_pixel(x, ROW) == fill)
        .collect();
    assert!(!xs.is_empty(), "{name}: no thumb on row {ROW}");
    let scale = image.width() as f32 / WIDTH;
    xs.iter().sum::<u32>() as f32 / xs.len() as f32 / scale
}

fn open() -> (App, Entity<Host>, image::Rgba<u8>) {
    let (mut app, host) = support::open(40., |_, _| Host { selected: 0 });
    let first = app.screenshot("segmented-thumb-rest");
    let fill = *first.get_pixel(first.width() / 8, ROW);
    (app, host, fill)
}

#[test]
fn the_selection_thumb_slides_to_the_new_segment_over_the_tab_change() {
    let (mut app, host, fill) = open();
    let segment = WIDTH / 4.;
    let from = thumb_x(&mut app, "segmented-thumb-0", fill);
    assert!((from - segment / 2.).abs() < 3., "starts on One: {from}");

    select(&mut app, &host, 2);
    let start = thumb_x(&mut app, "segmented-thumb-1-start", fill);
    assert!((start - from).abs() < 3., "it leaves from One: {start}");
    app.cx.advance_clock(STEP_CHANGE.duration / 2);
    let mid = thumb_x(&mut app, "segmented-thumb-2-mid", fill);
    assert!(
        mid > from + 10. && mid < segment * 2.5 - 10.,
        "half way it is between One and Three: {mid}"
    );
    app.cx.advance_clock(STEP_CHANGE.duration);
    let end = thumb_x(&mut app, "segmented-thumb-3-end", fill);
    assert!((end - segment * 2.5).abs() < 3., "it lands on Three: {end}");
}

#[test]
fn with_reduce_motion_the_thumb_jumps() {
    let (mut app, host, fill) = open();
    app.cx.update(|cx| set_reduce_motion(true, cx));
    select(&mut app, &host, 3);
    let at = thumb_x(&mut app, "segmented-thumb-reduced", fill);
    assert!(
        (at - WIDTH * 7. / 8.).abs() < 3.,
        "it is on Four at once: {at}"
    );
}
