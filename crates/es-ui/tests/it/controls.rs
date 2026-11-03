use crate::support;

use std::ops::RangeInclusive;

use es_ui::{
    components::{InspectorSlider, Segment, Segmented, SliderEvent, ValueFormat},
    theme,
};
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use support::*;

struct SliderHost {
    slider: Entity<InspectorSlider>,
    events: Vec<SliderEvent>,
}

impl Render for SliderHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.slider.clone())
    }
}

fn slider(
    value: f64,
    range: RangeInclusive<f64>,
    format: ValueFormat,
) -> (App, Entity<SliderHost>) {
    open(80., move |window, cx| {
        let slider = cx.new(|cx| InspectorSlider::new("Tilt", value, range, format, window, cx));
        cx.subscribe(&slider, |host: &mut SliderHost, _, event, _| {
            host.events.push(event.clone())
        })
        .detach();
        SliderHost {
            slider,
            events: Vec::new(),
        }
    })
}

fn track_width(app: &mut App) -> f32 {
    let value = app
        .cx
        .update(|cx| theme::tokens(cx).metrics.slider_value_width);
    WIDTH - GAP - f32::from(value)
}

fn value(app: &mut App, host: &Entity<SliderHost>) -> f64 {
    app.cx.update(|cx| host.read(cx).slider.read(cx).value())
}

fn take_events(app: &mut App, host: &Entity<SliderHost>) -> Vec<SliderEvent> {
    app.cx
        .update(|cx| host.update(cx, |host, _| std::mem::take(&mut host.events)))
}

fn field_centre(app: &mut App) -> (f32, f32) {
    let value = app
        .cx
        .update(|cx| theme::tokens(cx).metrics.slider_value_width);
    (WIDTH - f32::from(value) / 2., 16.)
}

fn assert_close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() < 1e-3, "{what}: got {got}, want {want}");
}

#[test]
fn slider_drag_maps_pointer_x_across_an_unsigned_range() {
    let (mut app, host) = slider(0.5, 0.0..=1.0, ValueFormat::percent());
    let w = track_width(&mut app);

    press(&mut app, w * 0.25, 16.);
    assert_close(
        value(&mut app, &host),
        0.25,
        "pressing jumps to the pointer",
    );
    drag(&mut app, w * 0.75, 16.);
    assert_close(value(&mut app, &host), 0.75, "dragging follows the pointer");
    drag(&mut app, w + 80., 16.);
    assert_close(
        value(&mut app, &host),
        1.0,
        "past the end clamps to the maximum",
    );
    drag(&mut app, -40., 16.);
    assert_close(
        value(&mut app, &host),
        0.0,
        "before the start clamps to the minimum",
    );
    release(&mut app, -40., 16.);
}

#[test]
fn signed_slider_snaps_to_zero_within_three_pixels_and_keeps_both_sides_reachable() {
    let (mut app, host) = slider(20., -45.0..=45.0, ValueFormat::degrees().signed());
    let w = track_width(&mut app);
    let zero = w / 2.;

    for offset in [-2.9, -1., 0., 1.5, 2.9] {
        press(&mut app, zero + offset, 16.);
        release(&mut app, zero + offset, 16.);
        assert_eq!(
            value(&mut app, &host),
            0.,
            "{offset} px from zero is inside the detent"
        );
    }

    let side = zero - 3.;
    press(&mut app, zero + 3. + side / 2., 16.);
    assert_close(
        value(&mut app, &host),
        22.5,
        "right half remaps after the detent",
    );
    drag(&mut app, side / 2., 16.);
    assert_close(
        value(&mut app, &host),
        -22.5,
        "left half remaps before the detent",
    );
    drag(&mut app, zero + 3.5, 16.);
    let just_right = value(&mut app, &host);
    assert!(
        just_right > 0. && just_right < 0.2,
        "a value just past the detent is reachable, got {just_right}"
    );
    release(&mut app, zero + 3.5, 16.);
}

#[test]
fn slider_drag_is_bracketed_so_one_drag_can_be_one_undo_step() {
    let (mut app, host) = slider(0.0, 0.0..=1.0, ValueFormat::percent());
    let w = track_width(&mut app);

    press(&mut app, w * 0.5, 16.);
    drag(&mut app, w * 0.6, 16.);
    drag(&mut app, w * 0.7, 16.);
    release(&mut app, w * 0.7, 200.);

    let events = take_events(&mut app, &host);
    assert_eq!(events.first(), Some(&SliderEvent::DragStarted));
    assert_eq!(
        events.last(),
        Some(&SliderEvent::DragEnded),
        "releasing outside the track still ends the drag"
    );
    let changes = &events[1..events.len() - 1];
    assert_eq!(changes.len(), 3, "{events:?}");
    assert!(changes.iter().all(|e| matches!(e, SliderEvent::Changed(_))));

    drag(&mut app, w * 0.2, 16.);
    assert!(
        take_events(&mut app, &host).is_empty(),
        "moving after release changes nothing"
    );
    assert_close(value(&mut app, &host), 0.7, "the drag's last value stays");
}

#[test]
fn arrow_keys_step_the_track_by_the_format_step() {
    let (mut app, host) = slider(10., 0.0..=100.0, ValueFormat::pixels());
    let slider = app.cx.update(|cx| host.read(cx).slider.clone());
    focus(&mut app, &slider);

    key(&mut app, "right");
    key(&mut app, "right");
    key(&mut app, "left");
    assert_close(value(&mut app, &host), 11., "Right twice, Left once");
    assert_eq!(
        take_events(&mut app, &host),
        vec![
            SliderEvent::Changed(11.),
            SliderEvent::Changed(12.),
            SliderEvent::Changed(11.)
        ],
        "keyboard steps are single edits, outside any drag bracket"
    );

    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            slider.update(cx, |s, cx| s.set_value(100., window, cx))
        })
        .expect("window is open");
    app.settle();
    assert!(
        take_events(&mut app, &host).is_empty(),
        "a model update is not echoed back"
    );
    focus(&mut app, &slider);
    key(&mut app, "right");
    assert_close(
        value(&mut app, &host),
        100.,
        "stepping clamps at the maximum",
    );
    assert!(
        take_events(&mut app, &host).is_empty(),
        "a clamped step changes nothing"
    );
}

#[test]
fn value_field_commits_on_enter_reverts_on_escape_and_parses_suffixes() {
    let (mut app, host) = slider(1.5, 1.0..=3.0, ValueFormat::magnification(2));
    let (x, y) = field_centre(&mut app);

    click(&mut app, x, y);
    type_text(&mut app, "2.5x");
    assert_close(value(&mut app, &host), 1.5, "typing alone changes nothing");
    key(&mut app, "enter");
    assert_close(
        value(&mut app, &host),
        2.5,
        "Enter commits a suffixed value",
    );
    assert_eq!(
        take_events(&mut app, &host),
        vec![SliderEvent::Changed(2.5)]
    );

    click(&mut app, x, y);
    type_text(&mut app, "1.25");
    key(&mut app, "escape");
    assert_close(value(&mut app, &host), 2.5, "Escape reverts");
    assert!(
        take_events(&mut app, &host).is_empty(),
        "Escape emits nothing"
    );

    click(&mut app, x, y);
    type_text(&mut app, "nine");
    key(&mut app, "enter");
    assert_close(
        value(&mut app, &host),
        2.5,
        "text that is not a number is ignored",
    );

    click(&mut app, x, y);
    type_text(&mut app, "9");
    key(&mut app, "enter");
    assert_close(
        value(&mut app, &host),
        3.0,
        "a typed value clamps to the range",
    );
}

#[test]
fn up_and_down_step_the_value_field() {
    let (mut app, host) = slider(40., 0.0..=100.0, ValueFormat::pixels());
    let (x, y) = field_centre(&mut app);

    click(&mut app, x, y);
    key(&mut app, "up");
    key(&mut app, "up");
    assert_close(value(&mut app, &host), 42., "Up steps the field");
    key(&mut app, "down");
    assert_close(value(&mut app, &host), 41., "Down steps the field");

    type_text(&mut app, "60");
    key(&mut app, "up");
    assert_close(
        value(&mut app, &host),
        61.,
        "Up commits what was typed, then steps",
    );
}

#[test]
fn value_formats_display_and_parse_their_units() {
    let percent = ValueFormat::percent();
    assert_eq!(percent.display(0.45).as_ref(), "45%");
    assert_eq!(percent.parse("45%"), Some(0.45));
    assert_eq!(percent.parse(" 45 "), Some(0.45));

    let degrees = ValueFormat::degrees().signed();
    assert_eq!(degrees.display(12.).as_ref(), "+12°");
    assert_eq!(degrees.display(0.).as_ref(), "0°", "zero carries no sign");
    assert_eq!(
        degrees.parse("−12 deg"),
        Some(-12.),
        "a typographic minus parses"
    );
    assert_eq!(degrees.parse("-12°"), Some(-12.));

    let pixels = ValueFormat::pixels();
    assert_eq!(pixels.parse("24 PX"), Some(24.), "suffixes ignore case");
    assert_eq!(pixels.parse("24 pt"), None, "another unit is refused");

    let named = ValueFormat::integer().named(0., "Off");
    assert_eq!(named.display(0.).as_ref(), "Off");
    assert_eq!(named.parse("off"), Some(0.));
    assert_eq!(ValueFormat::seconds().parse("inf"), None);
}

#[test]
fn disabled_slider_ignores_pointer_and_keys() {
    let (mut app, host) = slider(0.5, 0.0..=1.0, ValueFormat::percent());
    let slider = app.cx.update(|cx| host.read(cx).slider.clone());
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            slider.update(cx, |s, cx| s.set_disabled(true, window, cx))
        })
        .expect("window is open");
    app.settle();
    let w = track_width(&mut app);

    press(&mut app, w * 0.1, 16.);
    release(&mut app, w * 0.1, 16.);
    focus(&mut app, &slider);
    key(&mut app, "left");
    assert_close(value(&mut app, &host), 0.5, "nothing moved it");
    assert!(take_events(&mut app, &host).is_empty());
}

struct SegmentHost {
    selected: usize,
}

impl Render for SegmentHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().size_full().child(
            Segmented::new(
                "style",
                "Shadow style",
                ["Soft", "Long", "Glow", "Crisp"].map(Segment::text),
            )
            .selected(self.selected)
            .on_select(move |index, _, cx| {
                this.update(cx, |host, cx| {
                    host.selected = index;
                    cx.notify();
                })
            }),
        )
    }
}

#[test]
fn segmented_selects_by_click_and_by_arrow_keys() {
    let (mut app, host) = open(80., |_, _| SegmentHost { selected: 0 });
    let selected = |app: &mut App| app.cx.update(|cx| host.read(cx).selected);
    let segment = WIDTH / 4.;

    click(&mut app, segment * 2.5, 16.);
    assert_eq!(selected(&mut app), 2, "clicking Glow selects it");

    key(&mut app, "right");
    assert_eq!(selected(&mut app), 3, "Right moves to the next segment");
    key(&mut app, "right");
    assert_eq!(selected(&mut app), 3, "Right stops at the last segment");
    key(&mut app, "up");
    key(&mut app, "left");
    assert_eq!(selected(&mut app), 1, "Up and Left move back");
    key(&mut app, "left");
    key(&mut app, "left");
    assert_eq!(selected(&mut app), 0, "Left stops at the first segment");
    key(&mut app, "down");
    assert_eq!(selected(&mut app), 1, "Down moves forward");
}
