use crate::support;

use std::{cell::RefCell, rc::Rc, time::Duration};

use es_ui::motion::{TOOLTIP_DELAY, TOOLTIP_WARM};
use gpui::{
    Context, InteractiveElement, IntoElement, MouseMoveEvent, ParentElement, PlatformInput, Render,
    StatefulInteractiveElement, Styled, Window, div, point, px,
};
use gpui_component::tooltip::{Tooltip, TooltipDefaults};
use support::*;

const MS: Duration = Duration::from_millis(1);

struct Pair {
    shown: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Pair {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let tip = |name: &'static str, left: f32| {
            let shown = self.shown.clone();
            div()
                .id(name)
                .absolute()
                .left(px(left))
                .top(px(10.))
                .size(px(60.))
                .tooltip(move |window, cx| {
                    shown.borrow_mut().push(name);
                    Tooltip::new(name).build(window, cx)
                })
        };
        div().size_full().child(tip("a", 10.)).child(tip("b", 80.))
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

fn last(shown: &Rc<RefCell<Vec<&'static str>>>) -> Option<&'static str> {
    shown.borrow().last().copied()
}

#[test]
fn a_tooltip_waits_its_token_delay_and_its_neighbour_opens_at_once_while_warm() {
    let shown = Rc::new(RefCell::new(Vec::new()));
    let (mut app, _) = open(120., |_, _| Pair {
        shown: shown.clone(),
    });

    hover(&mut app, 30.);
    wait(&mut app, TOOLTIP_DELAY - MS);
    assert_eq!(last(&shown), None, "a cold tooltip waits out its delay");
    wait(&mut app, MS);
    assert_eq!(last(&shown), Some("a"));

    hover(&mut app, 100.);
    assert_eq!(
        last(&shown),
        Some("b"),
        "moving straight to a neighbour shows its tooltip with no delay"
    );

    hover(&mut app, 200.);
    wait(&mut app, TOOLTIP_WARM - 10 * MS);
    hover(&mut app, 30.);
    assert_eq!(
        last(&shown),
        Some("a"),
        "the tooltips stay warm for the warm window after the pointer leaves"
    );

    hover(&mut app, 200.);
    wait(&mut app, TOOLTIP_WARM);
    hover(&mut app, 100.);
    wait(&mut app, TOOLTIP_DELAY - MS);
    assert_eq!(
        shown.borrow().len(),
        3,
        "once the warm window has passed, the delay applies again"
    );
    wait(&mut app, MS);
    assert_eq!(last(&shown), Some("b"));
}

#[test]
fn the_kit_tooltips_use_the_same_tokens() {
    let (mut app, _) = open(120., |_, _| Pair {
        shown: Rc::default(),
    });
    let defaults = app.cx.update(|cx| TooltipDefaults::global(cx));
    assert_eq!(
        (defaults.show_delay(), defaults.grace_period()),
        (TOOLTIP_DELAY, TOOLTIP_WARM)
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
