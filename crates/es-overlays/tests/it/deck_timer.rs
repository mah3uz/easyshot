#![allow(clippy::unwrap_used)]
use crate::deck::{Rig, rig_with};
use es_app::deck::{DeckAction, DeckPolicy};
use es_model::config::CardActions;
use es_overlays::deck::view::DeckSettings;
use gpui::{Modifiers, MouseExitEvent, PlatformInput, point, px};
use std::time::Duration;

const DELAY: Duration = Duration::from_secs(2);

fn timed(r: &mut Rig) {
    let deck = r.deck.clone();
    r.s.app.cx.update(|cx| {
        deck.update(cx, |d, cx| {
            d.set_policy(
                DeckPolicy {
                    delay: Some(DELAY),
                    keep_until_saved: false,
                },
                cx,
            )
        })
    });
}

fn leave(r: &mut Rig, x: f32, y: f32) {
    r.s.send(PlatformInput::MouseExited(MouseExitEvent {
        position: point(px(x), px(y)),
        pressed_button: None,
        modifiers: Modifiers::default(),
    }));
    r.next_frame();
}

fn wait(r: &mut Rig, time: Duration) {
    r.s.app.cx.advance_clock(time);
    r.s.app.settle();
}

#[test]
fn the_other_cards_go_after_the_one_under_the_pointer_is_copied_and_the_pointer_leaves() {
    let mut r = rig_with(3, false, DeckSettings::default(), None);
    timed(&mut r);
    let c = r.card(0);
    let (x, y) = ((c.center().x - 22.) as f32, c.center().y as f32);
    r.click(x, y);
    assert_eq!(r.requested(), [("shot1".into(), DeckAction::Copy)]);
    assert_eq!(r.names(), ["shot2", "shot3"]);
    leave(&mut r, x, y);
    let deck = r.deck.clone();
    assert!(
        !r.s.app.cx.update(|cx| deck.read(cx).hovered()),
        "the pointer is on another surface, so nothing holds the timers"
    );
    wait(&mut r, DELAY + Duration::from_millis(300));
    assert!(r.names().is_empty(), "{:?}", r.names());
}

#[test]
fn a_card_whose_button_was_clicked_goes_once_the_pointer_leaves() {
    let settings = DeckSettings {
        actions: CardActions {
            preview: true,
            ..CardActions::default()
        },
        ..DeckSettings::default()
    };
    let mut r = rig_with(1, false, settings, Some("shot1.png"));
    timed(&mut r);
    let c = r.card(0);
    let (x, y) = (c.center().x as f32, (c.y0 + 19.) as f32);
    r.click(x, y);
    assert_eq!(r.requested(), [("shot1".into(), DeckAction::Preview)]);
    leave(&mut r, x, y);
    wait(&mut r, DELAY - Duration::from_millis(300));
    assert_eq!(r.names(), ["shot1"], "the full delay runs after leaving");
    wait(&mut r, Duration::from_millis(600));
    assert!(
        r.names().is_empty(),
        "a click leaves the card focused, but only a keyboard user holds it there"
    );
}

#[test]
fn a_card_the_keyboard_is_on_waits() {
    let mut r = rig_with(1, false, DeckSettings::default(), Some("shot1.png"));
    timed(&mut r);
    let c = r.card(0);
    let (x, y) = (c.center().x as f32, (c.y0 + 19.) as f32);
    r.point(x, y);
    r.s.down(x, y);
    leave(&mut r, x, y);
    r.s.key("shift");
    r.next_frame();
    r.next_frame();
    wait(&mut r, DELAY * 3);
    assert_eq!(r.names(), ["shot1"]);
}
