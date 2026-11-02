mod support;

use std::{cell::Cell, rc::Rc};

use es_model::background::Alignment;
use es_ui::{
    components::{AlignmentPad, CENTRE, DisclosureSection, EditorButton, FocusPad, Popover},
    motion,
};
use gpui::{
    Context, IntoElement, KeyUpEvent, MouseButton, MouseDownEvent, ParentElement, PlatformInput,
    Point, Render, Styled, Window, canvas, div, point, px,
};
use support::*;

/// Paints a marker that records it was drawn, so a test can tell whether content is mounted.
fn painted(flag: &Rc<Cell<bool>>) -> impl IntoElement {
    let flag = flag.clone();
    canvas(|_, _, _| (), move |_, _, _, _| flag.set(true)).size(px(10.))
}

/// Draws a fresh frame and reports whether the marker was in it, so frames from earlier input do not count.
fn drawn_now(app: &mut App, flag: &Rc<Cell<bool>>) -> bool {
    flag.set(false);
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .expect("window is open");
    app.settle();
    flag.get()
}

/// A keyboard click needs the key to come back up, which `key` alone does not send.
fn activate(app: &mut App, keystroke: &str) {
    key(app, keystroke);
    send(
        app,
        PlatformInput::KeyUp(KeyUpEvent {
            keystroke: stroke(keystroke),
        }),
    );
}

fn still(app: &mut App) {
    app.cx.update(|cx| motion::set_reduce_motion(true, cx));
    app.settle();
}

struct SectionHost {
    expanded: bool,
    drawn: Rc<Cell<bool>>,
}

impl Render for SectionHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().w(px(WIDTH)).child(
            DisclosureSection::new("section", "Camera")
                .expanded(self.expanded)
                .on_toggle(move |expanded, _, cx| {
                    this.update(cx, |host, cx| {
                        host.expanded = expanded;
                        cx.notify();
                    })
                })
                .child(painted(&self.drawn)),
        )
    }
}

#[test]
fn a_section_folds_by_click_and_by_keyboard_and_unmounts_its_content() {
    let drawn = Rc::new(Cell::new(false));
    let (mut app, host) = open(200., |_, _| SectionHost {
        expanded: false,
        drawn: drawn.clone(),
    });
    still(&mut app);
    let expanded = |app: &mut App| app.cx.update(|cx| host.read(cx).expanded);
    assert!(
        !drawn_now(&mut app, &drawn),
        "a collapsed section draws no content"
    );

    key(&mut app, "tab");
    activate(&mut app, "enter");
    assert!(
        expanded(&mut app),
        "Tab in a fresh window reaches the title, and Enter unfolds it"
    );
    assert!(
        drawn_now(&mut app, &drawn),
        "the content is drawn once expanded"
    );
    activate(&mut app, "space");
    assert!(!expanded(&mut app), "Space folds it");

    click(&mut app, 40., 19.);
    assert!(expanded(&mut app), "clicking the title expands");

    click(&mut app, WIDTH - 20., 19.);
    assert!(!expanded(&mut app), "the chevron folds it too");
    assert!(
        !drawn_now(&mut app, &drawn),
        "collapsed content is gone, not just hidden"
    );
}

struct PadHost {
    alignment: Alignment,
}

impl Render for PadHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().child(
            AlignmentPad::new("pad", "Image alignment")
                .selected(self.alignment)
                .on_select(move |alignment, _, cx| {
                    this.update(cx, |host, cx| {
                        host.alignment = alignment;
                        cx.notify();
                    })
                }),
        )
    }
}

#[test]
fn the_alignment_pad_picks_by_click_and_walks_the_grid_with_arrows() {
    let (mut app, host) = open(100., |_, _| PadHost {
        alignment: Alignment::Center,
    });
    let alignment = |app: &mut App| app.cx.update(|cx| host.read(cx).alignment);
    let cell = |i: f32| 2. + i * 14. + 6.;

    click(&mut app, cell(0.), cell(0.));
    assert_eq!(alignment(&mut app), Alignment::TopLeading);
    click(&mut app, cell(2.), cell(2.));
    assert_eq!(alignment(&mut app), Alignment::BottomTrailing);

    key(&mut app, "up");
    assert_eq!(alignment(&mut app), Alignment::Trailing);
    key(&mut app, "right");
    assert_eq!(
        alignment(&mut app),
        Alignment::Trailing,
        "Right at the edge stays put instead of wrapping to the next row"
    );
    key(&mut app, "left");
    key(&mut app, "left");
    assert_eq!(alignment(&mut app), Alignment::Leading);
}

struct FocusHost {
    position: Point<f32>,
}

impl Render for FocusHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().w(px(WIDTH)).child(
            FocusPad::new("focus", "Blur focus position")
                .position(self.position)
                .on_change(move |position, _, cx| {
                    this.update(cx, |host, cx| {
                        host.position = position;
                        cx.notify();
                    })
                }),
        )
    }
}

fn near(got: Point<f32>, want: (f32, f32)) -> bool {
    (got.x - want.0).abs() < 0.01 && (got.y - want.1).abs() < 0.01
}

#[test]
fn the_focus_pad_follows_a_drag_stays_on_the_pad_and_nudges_with_arrows() {
    let (mut app, host) = open(200., |_, _| FocusHost {
        position: point(0.1, 0.1),
    });
    let position = |app: &mut App| app.cx.update(|cx| host.read(cx).position);
    let height = 118.;

    press(&mut app, WIDTH / 2., height / 2.);
    assert!(near(position(&mut app), (0.5, 0.5)), "pressing jumps there");
    drag(&mut app, WIDTH * 0.75, height * 0.25);
    assert!(
        near(position(&mut app), (0.75, 0.25)),
        "{:?}",
        position(&mut app)
    );
    drag(&mut app, WIDTH + 80., 190.);
    assert!(
        near(position(&mut app), (1., 1.)),
        "dragging past the edge pins the point to the pad"
    );
    release(&mut app, WIDTH + 80., 190.);
    drag(&mut app, 10., 10.);
    assert!(
        near(position(&mut app), (1., 1.)),
        "after release the point no longer follows the pointer"
    );

    key(&mut app, "left");
    assert!(near(position(&mut app), (0.95, 1.)));
    key(&mut app, "shift-up");
    assert!(
        near(position(&mut app), (0.95, 0.99)),
        "Shift nudges finely"
    );

    send(
        &mut app,
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(20.), px(20.)),
            modifiers: Default::default(),
            click_count: 2,
            first_mouse: false,
        }),
    );
    release(&mut app, 20., 20.);
    assert!(
        near(position(&mut app), (CENTRE.x, CENTRE.y)),
        "double-click centres"
    );
}

struct PopoverHost {
    open: bool,
    drawn: Rc<Cell<bool>>,
}

impl Render for PopoverHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let drawn = self.drawn.clone();
        div().size_full().child(
            Popover::new("popover", "Export Options")
                .trigger(EditorButton::new("export").label("Export"))
                .on_open_change(move |open, _, cx| {
                    this.update(cx, |host, cx| {
                        host.open = open;
                        cx.notify();
                    })
                })
                .content(move |_, _| div().child(painted(&drawn))),
        )
    }
}

#[test]
fn the_popover_opens_from_its_trigger_and_escape_closes_it() {
    let drawn = Rc::new(Cell::new(false));
    let (mut app, host) = open(300., |_, _| PopoverHost {
        open: false,
        drawn: drawn.clone(),
    });
    let is_open = |app: &mut App| app.cx.update(|cx| host.read(cx).open);

    click(&mut app, 20., 16.);
    assert!(is_open(&mut app), "clicking the trigger opens it");
    assert!(drawn_now(&mut app, &drawn), "the panel's content is drawn");

    key(&mut app, "escape");
    assert!(!is_open(&mut app), "Escape closes it");
    assert!(
        !drawn_now(&mut app, &drawn),
        "a closed popover draws nothing"
    );

    click(&mut app, 20., 16.);
    assert!(is_open(&mut app), "it opens again");
    click(&mut app, WIDTH - 10., 290.);
    assert!(!is_open(&mut app), "a click outside closes it");
}

#[test]
fn with_motion_on_a_folding_section_unmounts_once_its_close_has_played() {
    let drawn = Rc::new(Cell::new(false));
    let (mut app, host) = open(200., |_, _| SectionHost {
        expanded: true,
        drawn: drawn.clone(),
    });
    click(&mut app, 40., 19.);
    assert!(!app.cx.update(|cx| host.read(cx).expanded));
    assert!(
        drawn_now(&mut app, &drawn),
        "the content stays while it folds away"
    );

    app.cx.advance_clock(motion::SECTION_EXPAND.duration * 2);
    app.settle();
    assert!(
        !drawn_now(&mut app, &drawn),
        "once folded, the content leaves the tree and the Tab order"
    );
}
