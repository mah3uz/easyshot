#![allow(clippy::unwrap_used)]
use crate::deck::{Rig, rig_with};
use crate::deck_timer::{DELAY, leave, timed, wait};
use es_app::deck::DeckAction;
use es_geom::{Rect, Size};
use es_overlays::deck::layout::{CardSize, Corner, DeckGeometry, PEEK, PEEKING};
use es_overlays::deck::view::DeckSettings;
use es_ui::motion::{DECK_ENTER, DECK_ENTER_REDUCED, DECK_FAN, set_reduce_motion};
use std::time::Duration;
use uuid::Uuid;

fn geometry(count: usize, corner: Corner) -> DeckGeometry {
    DeckGeometry::new(CardSize::Small.size(), 20., corner, count)
}

fn inside(outer: Rect, inner: Rect) -> bool {
    outer.union(inner) == outer
}

#[test]
fn one_card_folds_to_itself_and_takes_only_its_own_rect() {
    let g = geometry(1, Corner::BottomRight);
    assert_eq!(g.folded(0), g.card(0));
    assert_eq!(g.badge(), None, "one card has nothing to count");
    assert_eq!(g.input_region(false), [g.card(0)]);
    assert_eq!(g.input_region(true), [g.card(0)]);
}

#[test]
fn folded_the_newest_card_stays_put_and_two_older_ones_peek_above_it() {
    for count in [3, 5] {
        let g = geometry(count, Corner::BottomRight);
        let front = g.card(count - 1);
        assert_eq!(
            g.folded(count - 1),
            front,
            "{count}: the front card does not move as the deck fans"
        );
        for i in 0..count - 1 {
            let r = g.folded(i);
            let depth = (count - 1 - i).min(PEEKING) as f64;
            assert_eq!(r.y0, front.y0 - PEEK * depth, "{count}: card {i} peeks");
            assert!(r.width() < front.width() && r.height() < front.height());
            assert_eq!(r.center().x, front.center().x, "the stack stays centred");
            assert!(r.y1 < front.y1, "the rest of it hides behind the front");
            assert_eq!(g.at(i, 0.), r);
            assert_eq!(g.at(i, 1.), g.card(i));
        }
        if count == 5 {
            assert_eq!(
                g.folded(0),
                g.folded(2),
                "past two, older cards lie under the last one that shows"
            );
        }
        let region = g.input_region(false);
        let stack = Rect::new(front.x0, front.y0 - 2. * PEEK, front.x1, front.y1);
        let badge = g.badge().unwrap();
        assert_eq!(
            region,
            [stack, badge],
            "{count}: folded, only the stack takes the pointer"
        );
        assert!(
            badge.center().x <= front.x0 + 2. && badge.y0 < stack.y0,
            "the count sits on the corner away from the screen edge"
        );
        let fanned = g.input_region(true);
        assert_eq!(fanned.len(), 1);
        for r in (0..count).map(|i| g.card(i)).chain(g.header()) {
            assert!(
                inside(fanned[0], r),
                "{count}: fanned, every card and the header take it"
            );
        }
        let panel = Rect::from_origin_size((0., 0.), g.panel());
        assert!(inside(panel, fanned[0]) && inside(panel, badge));
        assert_eq!(
            g.panel(),
            DeckGeometry::new(CardSize::Small.size(), 20., Corner::BottomRight, count).panel(),
        );
    }
    let left = geometry(3, Corner::BottomLeft);
    let badge = left.badge().unwrap();
    assert!(badge.center().x >= left.card(2).x1 - 2.);
    assert!(badge.x1 < left.panel().width);
    assert_eq!(left.card(2).size(), Size::new(180., 130.));
}

fn spread(r: &mut Rig) -> (f32, bool, bool) {
    let (view, deck) = (r.view.clone(), r.deck.clone());
    r.s.app.cx.update(|cx| {
        (
            view.read(cx).spread(),
            view.read(cx).fanned(),
            deck.read(cx).fanned(),
        )
    })
}

fn hovered(r: &mut Rig) -> Option<Uuid> {
    let view = r.view.clone();
    r.s.app.cx.update(|cx| view.read(cx).hovered())
}

#[test]
fn hovering_the_stack_fans_it_crossing_gaps_keeps_it_and_leaving_folds_it() {
    let mut r = rig_with(3, false, DeckSettings::default(), None);
    assert_eq!(spread(&mut r), (0., false, false));
    let top = r.card(0).center();
    r.point(top.x as f32, top.y as f32);
    assert_eq!(
        spread(&mut r),
        (0., false, false),
        "where the oldest card shows fanned, a folded deck lets the pointer through"
    );

    let front = r.card(2).center();
    r.point(front.x as f32, front.y as f32);
    let (drawn, view, deck) = spread(&mut r);
    assert!(view && deck, "the stack under the pointer fans out");
    assert!(
        drawn < 1.,
        "it spreads over {DECK_FAN:?}, not at once: {drawn}"
    );
    r.s.app.cx.advance_clock(DECK_FAN.duration);
    r.next_frame();
    assert_eq!(spread(&mut r), (1., true, true));

    let gap = (r.card(1).y1 + r.card(2).y0) / 2.;
    r.point(front.x as f32, gap as f32);
    assert_eq!(hovered(&mut r), None);
    assert_eq!(
        spread(&mut r),
        (1., true, true),
        "a gap between cards does not fold it"
    );
    r.point(top.x as f32, top.y as f32);
    assert_eq!(hovered(&mut r), Some(Uuid::from_u128(1)));

    leave(&mut r, top.x as f32, top.y as f32);
    r.next_frame();
    let (_, view, deck) = spread(&mut r);
    assert!(!view && !deck, "leaving the surface folds the deck");
    r.s.app.cx.advance_clock(DECK_FAN.duration);
    r.next_frame();
    assert_eq!(spread(&mut r), (0., false, false));

    r.fan();
    r.point(2., 2.);
    assert!(!spread(&mut r).1, "the pointer off the cards folds it too");
}

#[test]
fn reduce_motion_fans_and_folds_at_once() {
    let mut r = rig_with(5, true, DeckSettings::default(), None);
    r.s.app.cx.update(|cx| set_reduce_motion(true, cx));
    let front = r.card(4).center();
    r.point(front.x as f32, front.y as f32);
    r.next_frame();
    assert_eq!(spread(&mut r), (1., true, true));
    r.point(2., 2.);
    r.next_frame();
    assert_eq!(spread(&mut r), (0., false, false));
}

#[test]
fn an_action_reaches_a_back_card_only_once_the_deck_is_fanned() {
    let mut r = rig_with(5, false, DeckSettings::default(), None);
    let c = r.card(1).center();
    let copy = ((c.x - 22.) as f32, c.y as f32);
    r.click(copy.0, copy.1);
    assert!(
        r.requested().is_empty(),
        "folded, that card is not there to press"
    );

    r.fan();
    r.click(copy.0, copy.1);
    assert_eq!(r.requested(), [("shot2".into(), DeckAction::Copy)]);
    assert_eq!(r.names(), ["shot1", "shot3", "shot4", "shot5"]);
}

#[test]
fn folded_cards_still_expire_one_by_one_and_the_next_comes_to_the_front() {
    let mut r = rig_with(3, false, DeckSettings::default(), None);
    let deck = r.deck.clone();
    let reshow = |r: &mut Rig, n: u128, auto_dismiss: bool| {
        r.s.app.cx.update(|cx| {
            deck.update(cx, |d, cx| {
                let mut item = d.get(Uuid::from_u128(n)).cloned().unwrap();
                item.auto_dismiss = auto_dismiss;
                d.show(item, cx);
            })
        })
    };
    reshow(&mut r, 1, false);
    reshow(&mut r, 2, true);
    reshow(&mut r, 3, true);
    timed(&mut r);
    wait(&mut r, Duration::from_secs(1));
    reshow(&mut r, 3, true);
    wait(&mut r, DELAY - Duration::from_millis(700));
    assert_eq!(
        r.names(),
        ["shot1", "shot3"],
        "a card behind the front goes on its own clock while the deck is folded"
    );
    wait(&mut r, Duration::from_secs(1));
    assert_eq!(
        r.names(),
        ["shot1"],
        "the front card goes too, and the one kept until closed comes to the front"
    );
    r.next_frame();
    assert_eq!(spread(&mut r), (0., false, false));
}

#[test]
fn pictures_of_the_stack_folded_and_fanned() {
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        for count in [1, 3, 5] {
            let mut r = rig_with(count, dark, DeckSettings::default(), None);
            r.s.app.screenshot(&format!("deck-stack-{count}-{theme}"));
        }
        let mut r = rig_with(5, dark, DeckSettings::default(), None);
        let front = r.card(4).center();
        r.point(front.x as f32, front.y as f32);
        r.s.app.cx.advance_clock(DECK_FAN.duration / 4);
        r.next_frame();
        r.s.app.screenshot(&format!("deck-fanning-5-{theme}"));
        r.s.app.cx.advance_clock(DECK_FAN.duration);
        r.next_frame();
        r.s.app.screenshot(&format!("deck-fanned-5-{theme}"));
    }
}

/// The rows of the blue band at the picture's left, which only the card draws, and how blue.
fn picture_rows(image: &image::RgbaImage) -> Option<(u32, u32, u32)> {
    let rows: Vec<(u32, u32)> = (0..image.height())
        .filter_map(|y| {
            let blue = (0..image.width())
                .map(|x| {
                    let p = image.get_pixel(x, y).0;
                    u32::from(p[2]).saturating_sub(u32::from(p[0]))
                })
                .max()?;
            (blue > 30).then_some((y, blue))
        })
        .collect();
    let strongest = rows.iter().map(|r| r.1).max()?;
    Some((rows.first()?.0, rows.last()?.0, strongest))
}

fn arriving(reduce: bool) -> (Rig, tempfile::TempDir) {
    let mut r = rig_with(0, true, DeckSettings::default(), None);
    r.s.app.cx.update(|cx| set_reduce_motion(reduce, cx));
    let dir = tempfile::tempdir().unwrap();
    let deck = r.deck.clone();
    let shot = crate::deck::item(dir.path(), 9);
    r.s.app
        .cx
        .update(|cx| deck.update(cx, |d, cx| d.show(shot, cx)));
    let view = r.view.clone();
    r.s.app.until("the thumbnail loads", |cx| {
        view.read(cx).has_image(Uuid::from_u128(9))
    });
    (r, dir)
}

#[test]
fn a_new_card_grows_and_rises_into_place_as_it_fades_in_and_only_fades_under_reduce_motion() {
    let (mut r, _dir) = arriving(false);
    r.s.app.cx.advance_clock(DECK_ENTER.duration / 8);
    r.next_frame();
    let early = picture_rows(&r.s.app.screenshot("deck-enter-early")).unwrap();
    r.s.app.cx.advance_clock(DECK_ENTER.duration);
    r.next_frame();
    let settled = picture_rows(&r.s.app.screenshot("deck-enter-settled")).unwrap();
    assert!(
        early.1 - early.0 + 4 < settled.1 - settled.0,
        "the card starts smaller than its place: {early:?} then {settled:?}"
    );
    assert!(
        early.1 > settled.1,
        "and lower, rising into it: {early:?} then {settled:?}"
    );
    assert!(early.2 < settled.2, "and faint, fading in");

    let (mut still, _dir) = arriving(true);
    still
        .s
        .app
        .cx
        .advance_clock(DECK_ENTER_REDUCED.duration / 2);
    still.next_frame();
    let fading = picture_rows(&still.s.app.screenshot("deck-enter-reduced")).unwrap();
    assert_eq!(
        (fading.0, fading.1),
        (settled.0, settled.1),
        "under Reduce Motion the card is drawn in its place at once"
    );
    assert!(fading.2 < settled.2, "and only fades in");
}
