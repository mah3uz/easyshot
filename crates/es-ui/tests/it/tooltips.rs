use crate::support;

use std::time::Duration;

use es_ui::components::{Tip, tooltip::SHOW_DELAY};
use gpui::{
    Context, InteractiveElement, IntoElement, MouseMoveEvent, ParentElement, PlatformInput, Render,
    StatefulInteractiveElement, Styled, Window, div, point, px,
};
use gpui_component::tooltip::TooltipDefaults;
use image::RgbaImage;
use support::*;

const MS: Duration = Duration::from_millis(1);
const REST: Duration = Duration::from_millis(1500);
const SHORT: &str = "Crop";
const SHORT_READ: Duration = Duration::from_millis(2000 + 60 * 4);
const READ_CAP: Duration = Duration::from_secs(8);
const LONG: &str = "Draws a box with a soft shadow around the part of the picture you want people to look at first, and dims the rest";

struct Pair {
    a: &'static str,
}

impl Render for Pair {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tip = |name: &'static str, text: &'static str, left: f32| {
            div()
                .id(name)
                .absolute()
                .left(px(left))
                .top(px(10.))
                .size(px(60.))
                .tip(text)
        };
        div()
            .size_full()
            .child(tip("a", self.a, 10.))
            .child(tip("b", "Rotate", 80.))
    }
}

fn hover(app: &mut App, x: f32) {
    send(
        app,
        PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(40.)),
            pressed_button: None,
            modifiers: Default::default(),
        }),
    );
}

fn wait(app: &mut App, by: Duration) {
    app.cx.advance_clock(by);
    app.settle();
}

fn open_pair(a: &'static str) -> (App, RgbaImage) {
    let (mut app, _) = open(120., |_, _| Pair { a });
    let rest = app.screenshot("tooltip-rest");
    (app, rest)
}

fn showing(app: &mut App, rest: &RgbaImage) -> bool {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .expect("window is open");
    app.screenshot("tooltip-now") != *rest
}

#[test]
fn every_tooltip_waits_the_rest_the_user_chose() {
    assert_eq!(SHOW_DELAY, REST);
}

#[test]
fn a_tooltip_shows_only_after_a_rest_of_a_second_and_a_half() {
    let (mut app, rest) = open_pair(SHORT);
    hover(&mut app, 30.);
    wait(&mut app, REST - MS);
    assert!(
        !showing(&mut app, &rest),
        "nothing shows before the rest is over"
    );
    wait(&mut app, MS);
    assert!(
        showing(&mut app, &rest),
        "it shows once the pointer has rested"
    );
}

#[test]
fn moving_straight_to_a_neighbour_waits_the_whole_rest_again() {
    let (mut app, rest) = open_pair(SHORT);
    hover(&mut app, 30.);
    wait(&mut app, REST);
    assert!(showing(&mut app, &rest));

    hover(&mut app, 100.);
    wait(&mut app, REST - MS);
    assert!(
        !showing(&mut app, &rest),
        "no tooltip jumps across to the next control"
    );
    wait(&mut app, MS);
    assert!(
        showing(&mut app, &rest),
        "the neighbour's own shows after its rest"
    );
}

#[test]
fn a_short_tooltip_hides_after_two_seconds_and_sixty_milliseconds_a_character() {
    let (mut app, rest) = open_pair(SHORT);
    hover(&mut app, 30.);
    wait(&mut app, REST);
    wait(&mut app, SHORT_READ - MS);
    assert!(showing(&mut app, &rest), "it stays up while it is read");
    wait(&mut app, MS);
    assert!(!showing(&mut app, &rest), "then it hides by itself");
}

#[test]
fn a_long_tooltip_hides_after_eight_seconds_at_most() {
    assert!(
        Duration::from_millis(2000 + 60 * LONG.chars().count() as u64) > READ_CAP,
        "long enough that the cap applies"
    );
    let (mut app, rest) = open_pair(LONG);
    hover(&mut app, 30.);
    wait(&mut app, REST);
    wait(&mut app, READ_CAP - MS);
    assert!(
        showing(&mut app, &rest),
        "a long one gets the full eight seconds"
    );
    wait(&mut app, MS);
    assert!(!showing(&mut app, &rest), "and no more");
}

#[test]
fn a_hidden_tooltip_comes_back_only_after_the_pointer_leaves_and_rests_again() {
    let (mut app, rest) = open_pair(SHORT);
    hover(&mut app, 30.);
    wait(&mut app, REST + SHORT_READ);
    assert!(!showing(&mut app, &rest));

    hover(&mut app, 32.);
    wait(&mut app, REST * 2);
    assert!(
        !showing(&mut app, &rest),
        "moving about on the same control does not bring it back"
    );

    hover(&mut app, 200.);
    wait(&mut app, 10 * MS);
    hover(&mut app, 30.);
    wait(&mut app, REST - MS);
    assert!(!showing(&mut app, &rest), "coming back waits the full rest");
    wait(&mut app, MS);
    assert!(showing(&mut app, &rest), "and then it shows again");
}

#[test]
fn the_kit_tooltips_use_the_same_rest_and_never_stay_warm() {
    let (mut app, _) = open_pair(SHORT);
    let defaults = app.cx.update(|cx| TooltipDefaults::global(cx));
    assert_eq!(
        (defaults.show_delay(), defaults.grace_period()),
        (REST, Duration::ZERO)
    );
}

struct Fill;

impl Render for Fill {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (amount, on_hover) = es_ui::motion::hover_fade("fill", window, cx);
        div().size_full().child(
            div()
                .id("fill")
                .absolute()
                .left(px(10.))
                .top(px(10.))
                .size(px(60.))
                .bg(gpui::white().opacity(amount))
                .on_hover(on_hover),
        )
    }
}

fn fill_at(app: &mut App) -> u8 {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .expect("window is open");
    app.screenshot("hover-fill").get_pixel(80, 80).0[0]
}

#[test]
fn a_hover_fill_fades_in_over_its_token_and_out_again_even_under_reduce_motion() {
    use es_ui::motion::{HOVER_FILL, set_reduce_motion};
    for reduce in [false, true] {
        let (mut app, _) = open(100., |_, _| Fill);
        app.cx.update(|cx| set_reduce_motion(reduce, cx));
        let rest = fill_at(&mut app);
        hover(&mut app, 40.);
        wait(&mut app, HOVER_FILL.duration / 2);
        let mid = fill_at(&mut app);
        wait(&mut app, HOVER_FILL.duration / 2 - MS * 4);
        let late = fill_at(&mut app);
        wait(&mut app, MS * 4);
        let full = fill_at(&mut app);
        assert!(
            rest < mid && mid < late && late < full,
            "{rest} {mid} {late} {full}"
        );
        hover(&mut app, 150.);
        wait(&mut app, HOVER_FILL.duration);
        assert_eq!(fill_at(&mut app), rest, "the fill leaves with the pointer");
    }
}

struct Options;

impl Render for Options {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use es_ui::components::{Segment, Segmented};
        div().w(px(200.)).child(Segmented::new(
            "codec",
            "Codec",
            [
                Segment::text("AV1").help("The smallest files."),
                Segment::text("MP4"),
            ],
        ))
    }
}

#[test]
fn an_option_with_help_shows_it_on_hover_and_one_without_shows_nothing() {
    let (mut app, _) = open(120., |_, _| Options);
    assert!(
        !tooltip_shows_at(&mut app, 150., 12.),
        "an option whose label says enough has no tooltip"
    );
    assert!(
        tooltip_shows_at(&mut app, 50., 12.),
        "an option with help shows it after the delay"
    );
}
