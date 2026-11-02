#![allow(clippy::unwrap_used)]
use es_app::deck::{
    Deck, DeckAction, DeckEvent, DeckItem, DeckPolicy, ItemStatus, MAX_ITEMS, Removal,
};
use es_model::CaptureKind;
use gpui::{AppContext as _, Entity, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use uuid::Uuid;

type Log = Rc<RefCell<Vec<DeckEvent>>>;

fn item(n: u32) -> DeckItem {
    DeckItem {
        id: Uuid::from_u128(u128::from(n)),
        name: format!("shot{n}"),
        file: PathBuf::from(format!("/deck/{n}/shot{n}.png")),
        thumb: PathBuf::from(format!("/thumbs/{n}.png")),
        kind: CaptureKind::Region,
        video: false,
        staged: true,
        status: ItemStatus::Idle,
        auto_dismiss: true,
    }
}

fn five_seconds() -> DeckPolicy {
    DeckPolicy {
        delay: Some(Duration::from_secs(5)),
        keep_until_saved: false,
    }
}

fn deck(cx: &mut TestAppContext, policy: DeckPolicy) -> (Entity<Deck>, Log) {
    let deck = cx.new(|_| Deck::new(policy));
    let log = Log::default();
    let sink = log.clone();
    cx.update(|cx| {
        cx.subscribe(&deck, move |_, e: &DeckEvent, _| {
            sink.borrow_mut().push(e.clone())
        })
        .detach();
    });
    (deck, log)
}

fn order(deck: &Entity<Deck>, cx: &mut TestAppContext) -> Vec<String> {
    deck.read_with(cx, |d, _| d.items().map(|i| i.name.clone()).collect())
}

fn show(deck: &Entity<Deck>, n: u32, cx: &mut TestAppContext) {
    deck.update(cx, |d, cx| d.show(item(n), cx));
}

fn removed(log: &Log) -> Vec<(String, Removal)> {
    log.borrow()
        .iter()
        .filter_map(|e| match e {
            DeckEvent::Removed { item, why } => Some((item.name.clone(), *why)),
            DeckEvent::Requested { .. } => None,
        })
        .collect()
}

#[gpui::test]
fn captures_stack_newest_last_and_a_reshown_one_moves_without_duplicating(cx: &mut TestAppContext) {
    let (deck, _) = deck(
        cx,
        DeckPolicy {
            delay: None,
            keep_until_saved: false,
        },
    );
    for n in 1..=3 {
        show(&deck, n, cx);
    }
    assert_eq!(order(&deck, cx), ["shot1", "shot2", "shot3"]);
    show(&deck, 1, cx);
    assert_eq!(order(&deck, cx), ["shot2", "shot3", "shot1"]);
}

#[gpui::test]
fn a_full_deck_evicts_the_oldest_idle_card_and_says_so(cx: &mut TestAppContext) {
    let (deck, log) = deck(
        cx,
        DeckPolicy {
            delay: None,
            keep_until_saved: false,
        },
    );
    for n in [1, 2, 3, 1] {
        show(&deck, n, cx);
    }
    for n in 4..=7 {
        show(&deck, n, cx);
    }
    assert_eq!(MAX_ITEMS, 5);
    assert_eq!(
        order(&deck, cx),
        ["shot1", "shot4", "shot5", "shot6", "shot7"]
    );
    assert_eq!(
        removed(&log),
        [
            ("shot2".into(), Removal::Evicted),
            ("shot3".into(), Removal::Evicted)
        ]
    );

    deck.update(cx, |d, cx| d.remove(item(5).id, Removal::Dismissed, cx));
    assert_eq!(
        order(&deck, cx),
        ["shot1", "shot4", "shot6", "shot7"],
        "the rest keep their order"
    );

    deck.update(cx, |d, cx| d.act(item(1).id, DeckAction::Save, cx));
    show(&deck, 8, cx);
    show(&deck, 9, cx);
    assert_eq!(
        order(&deck, cx),
        ["shot1", "shot6", "shot7", "shot8", "shot9"],
        "a card that is saving is never evicted; the oldest idle one goes"
    );
}

#[gpui::test]
fn copy_edit_and_pin_ask_the_app_then_remove_the_card(cx: &mut TestAppContext) {
    let (deck, log) = deck(
        cx,
        DeckPolicy {
            delay: None,
            keep_until_saved: false,
        },
    );
    for n in 1..=4 {
        show(&deck, n, cx);
    }
    deck.update(cx, |d, cx| {
        d.act(item(1).id, DeckAction::Copy, cx);
        d.act(item(2).id, DeckAction::Edit, cx);
        d.act(item(3).id, DeckAction::Pin, cx);
        d.act(item(4).id, DeckAction::Dismiss, cx);
    });
    let requested: Vec<_> = log
        .borrow()
        .iter()
        .filter_map(|e| match e {
            DeckEvent::Requested { item, action } => Some((item.name.clone(), *action)),
            DeckEvent::Removed { .. } => None,
        })
        .collect();
    assert_eq!(
        requested,
        [
            ("shot1".into(), DeckAction::Copy),
            ("shot2".into(), DeckAction::Edit),
            ("shot3".into(), DeckAction::Pin)
        ],
        "dismissing needs nothing from the app"
    );
    assert_eq!(
        removed(&log),
        [
            ("shot1".into(), Removal::Copied),
            ("shot2".into(), Removal::Edited),
            ("shot3".into(), Removal::Pinned),
            ("shot4".into(), Removal::Dismissed)
        ]
    );
    assert!(!Removal::Copied.discards() && Removal::Dismissed.discards());
}

#[gpui::test]
fn save_keeps_the_card_until_the_app_answers(cx: &mut TestAppContext) {
    let (deck, log) = deck(cx, five_seconds());
    show(&deck, 1, cx);
    show(&deck, 2, cx);
    deck.update(cx, |d, cx| d.save_all(cx));
    deck.update(cx, |d, cx| d.save_all(cx));
    let saves = log
        .borrow()
        .iter()
        .filter(|e| {
            matches!(
                e,
                DeckEvent::Requested {
                    action: DeckAction::Save,
                    ..
                }
            )
        })
        .count();
    assert_eq!(saves, 2, "a card already saving is not asked twice");
    cx.executor().advance_clock(Duration::from_secs(30));
    assert_eq!(
        deck.read_with(cx, |d, _| d.len()),
        2,
        "saving cards never time out"
    );
    deck.update(cx, |d, cx| {
        d.saved(item(1).id, cx);
        d.save_failed(item(2).id, cx);
    });
    assert_eq!(order(&deck, cx), ["shot2"]);
    cx.executor().advance_clock(Duration::from_secs(30));
    assert_eq!(
        order(&deck, cx),
        ["shot2"],
        "a failed save keeps the capture on screen"
    );
}

#[gpui::test]
fn clear_all_removes_every_card_not_saving(cx: &mut TestAppContext) {
    let (deck, log) = deck(
        cx,
        DeckPolicy {
            delay: None,
            keep_until_saved: false,
        },
    );
    for n in 1..=3 {
        show(&deck, n, cx);
    }
    deck.update(cx, |d, cx| {
        d.act(item(2).id, DeckAction::Save, cx);
        d.clear_all(cx);
    });
    assert_eq!(order(&deck, cx), ["shot2"]);
    assert_eq!(
        removed(&log),
        [
            ("shot1".into(), Removal::Cleared),
            ("shot3".into(), Removal::Cleared)
        ]
    );
}

#[gpui::test]
fn a_card_dismisses_itself_after_the_delay(cx: &mut TestAppContext) {
    let (deck, log) = deck(cx, five_seconds());
    show(&deck, 1, cx);
    cx.executor().advance_clock(Duration::from_millis(4900));
    assert_eq!(deck.read_with(cx, |d, _| d.len()), 1);
    cx.executor().advance_clock(Duration::from_millis(200));
    assert_eq!(removed(&log), [("shot1".into(), Removal::Expired)]);
}

#[gpui::test]
fn hovering_holds_every_timer_and_leaving_restarts_it(cx: &mut TestAppContext) {
    let (deck, _) = deck(cx, five_seconds());
    show(&deck, 1, cx);
    cx.executor().advance_clock(Duration::from_secs(4));
    deck.update(cx, |d, cx| d.set_hovered(true, cx));
    cx.executor().advance_clock(Duration::from_secs(60));
    assert_eq!(deck.read_with(cx, |d, _| d.len()), 1);
    deck.update(cx, |d, cx| d.set_hovered(false, cx));
    cx.executor().advance_clock(Duration::from_millis(4800));
    assert_eq!(
        deck.read_with(cx, |d, _| d.len()),
        1,
        "the full delay runs again after leaving"
    );
    cx.executor().advance_clock(Duration::from_millis(400));
    assert!(deck.read_with(cx, |d, _| d.is_empty()));
}

#[gpui::test]
fn a_focused_card_waits_while_the_others_go(cx: &mut TestAppContext) {
    let (deck, _) = deck(cx, five_seconds());
    show(&deck, 1, cx);
    show(&deck, 2, cx);
    deck.update(cx, |d, cx| d.set_focused(Some(item(2).id), cx));
    cx.executor().advance_clock(Duration::from_secs(10));
    assert_eq!(order(&deck, cx), ["shot2"]);
}

#[gpui::test]
fn kept_until_saved_reopened_and_never_cards_do_not_time_out(cx: &mut TestAppContext) {
    let keep = DeckPolicy {
        keep_until_saved: true,
        ..five_seconds()
    };
    let (deck, _) = deck(cx, keep);
    show(&deck, 1, cx);
    let mut saved = item(2);
    saved.staged = false;
    deck.update(cx, |d, cx| d.show(saved, cx));
    cx.executor().advance_clock(Duration::from_secs(6));
    assert_eq!(order(&deck, cx), ["shot1"], "only the staged card is kept");

    let (deck, _) = deck_with_never(cx);
    show(&deck, 3, cx);
    let mut reopened = item(4);
    reopened.auto_dismiss = false;
    deck.update(cx, |d, cx| d.show(reopened, cx));
    cx.executor().advance_clock(Duration::from_secs(600));
    assert_eq!(order(&deck, cx), ["shot3", "shot4"]);

    deck.update(cx, |d, cx| d.set_policy(five_seconds(), cx));
    cx.executor().advance_clock(Duration::from_secs(6));
    assert_eq!(
        order(&deck, cx),
        ["shot4"],
        "a new delay applies to the cards already shown"
    );
}

fn deck_with_never(cx: &mut TestAppContext) -> (Entity<Deck>, Log) {
    let overlay = es_model::config::Overlay {
        dismiss_delay: es_app::deck::NEVER_DISMISS_SECS,
        ..Default::default()
    };
    let policy = DeckPolicy::from_settings(&overlay, &Default::default());
    assert_eq!(policy.delay, None);
    deck(cx, policy)
}
