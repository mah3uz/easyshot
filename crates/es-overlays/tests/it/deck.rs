#![allow(clippy::unwrap_used)]
use crate::window_mode_support;

use es_app::deck::{Deck, DeckAction, DeckEvent, DeckItem, DeckPolicy, ItemStatus, Removal};
use es_geom::{Point, Rect, Size};
use es_model::CaptureKind;
use es_overlays::deck::layout::{
    CardSize, Corner, DeckGeometry, LayoutPreset, Slot, ToolLayout, action_name,
};
use es_overlays::deck::view::{DeckSettings, DeckView};
use es_ui::tokens::ThemeMode;
use gpui::{
    AnyView, AppContext as _, Context, Entity, IntoElement, Modifiers, MouseExitEvent,
    ParentElement as _, PlatformInput, Render, Styled as _, Window, div, point, px, rgb,
};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;
use window_mode_support::{Surface, rgb as pixel};

#[test]
fn every_layout_edit_keeps_actions_unique_dismiss_reachable_and_survives_saving() {
    assert_eq!(ToolLayout::parse("standard"), ToolLayout::standard());
    assert_eq!(ToolLayout::parse("invalid"), ToolLayout::standard());
    for preset in [
        LayoutPreset::Standard,
        LayoutPreset::Sharing,
        LayoutPreset::Minimal,
    ] {
        let base = preset.layout().unwrap();
        assert_eq!(base.preset(), preset);
        assert_eq!(ToolLayout::parse(&base.to_setting()), base);
        for slot in Slot::ALL {
            for action in DeckAction::ALL.map(Some).into_iter().chain([None]) {
                let mut layout = base;
                layout.assign(action, slot);
                let placed: Vec<_> = layout.assignments().map(|(_, a)| a).collect();
                let mut unique = placed.clone();
                unique.sort_by_key(|a| action_name(*a));
                unique.dedup();
                assert_eq!(
                    unique.len(),
                    placed.len(),
                    "moving an action cannot duplicate it"
                );
                assert!(
                    placed.contains(&DeckAction::Dismiss),
                    "Dismiss must stay reachable"
                );
                assert_eq!(
                    ToolLayout::parse(&layout.to_setting()),
                    layout,
                    "custom layouts persist"
                );
            }
        }
    }
}

#[test]
fn moving_an_action_swaps_and_hiding_one_frees_its_slot() {
    let mut layout = ToolLayout::standard();
    layout.assign(Some(DeckAction::Share), Slot::TopLeft);
    assert_eq!(layout.get(Slot::TopLeft), Some(DeckAction::Share));
    assert_eq!(layout.get(Slot::BottomRight), Some(DeckAction::Pin));
    assert_eq!(layout.preset(), LayoutPreset::Custom);
    layout.assign(None, Slot::BottomRight);
    assert_eq!(layout.slot_of(DeckAction::Pin), None);
    layout.assign(Some(DeckAction::Pin), Slot::TopRight);
    assert_eq!(layout.get(Slot::TopRight), Some(DeckAction::Pin));
    assert_eq!(
        layout.get(Slot::BottomRight),
        Some(DeckAction::Dismiss),
        "a displaced Dismiss moves to the free slot"
    );
}

#[test]
fn a_damaged_layout_is_repaired_rather_than_dropped() {
    let damaged = r#"{"top-left":"copy","center-left":"copy","bottom-right":"future-action"}"#;
    let recovered = ToolLayout::parse(damaged);
    assert_eq!(
        recovered
            .assignments()
            .filter(|(_, a)| *a == DeckAction::Copy)
            .count(),
        1
    );
    assert_eq!(recovered.get(Slot::TopRight), Some(DeckAction::Dismiss));
}

#[test]
fn the_panel_grows_by_a_card_and_gap_per_capture_plus_the_header() {
    let card = Size::new(180., 98.);
    let height = |n| {
        DeckGeometry::new(card, 48., Corner::BottomRight, n)
            .panel()
            .height
    };
    let base = 98. + 48. + 24.;
    assert_eq!(height(0), base);
    assert_eq!(height(1), base);
    assert_eq!(height(3), base + 2. * 108. + 26.);
    assert_eq!(CardSize::parse("large").size(), Size::new(300., 216.));
    assert_eq!(CardSize::parse("bogus"), CardSize::Small);
}

#[test]
fn the_newest_card_sits_in_the_corner_the_deck_is_anchored_to() {
    let output = Rect::new(0., 0., 1280., 720.);
    let card = CardSize::Small.size();
    let right = DeckGeometry::new(card, 20., Corner::BottomRight, 3);
    let panel = right.panel();
    assert_eq!(
        right.origin(output),
        Point::new(1280. - panel.width, 720. - panel.height)
    );
    let newest = right.card(2);
    assert_eq!(
        (newest.x1, newest.y1),
        (panel.width - 20., panel.height - 20.)
    );
    assert!(
        right.card(0).y0 < right.card(1).y0,
        "older cards stack above"
    );
    assert_eq!(
        right.input_region(true),
        [right.header().unwrap().union(newest)],
        "fanned, the header, the three cards and the gaps between them take the pointer"
    );

    let left = DeckGeometry::new(card, 20., Corner::BottomLeft, 1);
    assert_eq!(left.origin(output).x, 0.);
    assert_eq!(left.card(0).x0, 20.);
    assert!(left.header().is_none(), "one card has no header");
    assert_eq!(
        DeckGeometry::new(card, 200., Corner::BottomLeft, 1).margin,
        48.
    );
}

pub(crate) struct Stage {
    pub(crate) content: AnyView,
    pub(crate) dark: bool,
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let desk = if self.dark {
            rgb(0x2a2d33)
        } else {
            rgb(0xe9ecf1)
        };
        div().size_full().bg(desk).child(self.content.clone())
    }
}

fn thumbnail(dir: &std::path::Path, n: u8) -> PathBuf {
    let path = dir.join(format!("thumb{n}.png"));
    let image = image::RgbaImage::from_fn(360, 220, |x, y| {
        let band = (y / 22) % 2 == 0;
        match (x < 60 + u32::from(n) * 40, band) {
            (true, _) => image::Rgba([40, 110, 230, 255]),
            (false, true) => image::Rgba([250, 250, 252, 255]),
            (false, false) => image::Rgba([225, 229, 236, 255]),
        }
    });
    image.save(&path).unwrap();
    path
}

pub(crate) fn item(dir: &std::path::Path, n: u8) -> DeckItem {
    DeckItem {
        id: Uuid::from_u128(u128::from(n)),
        name: format!("shot{n}"),
        file: dir.join(format!("shot{n}.png")),
        thumb: thumbnail(dir, n),
        kind: CaptureKind::Region,
        video: false,
        staged: true,
        status: ItemStatus::Idle,
        auto_dismiss: true,
        saved: None,
    }
}

pub(crate) struct Rig {
    pub(crate) s: Surface<Stage>,
    pub(crate) deck: Entity<Deck>,
    pub(crate) view: Entity<DeckView>,
    events: Rc<RefCell<Vec<DeckEvent>>>,
    pub(crate) geometry: DeckGeometry,
    _dir: tempfile::TempDir,
    _sub: gpui::Subscription,
}

fn rig(cards: u8, dark: bool) -> Rig {
    rig_with(cards, dark, DeckSettings::default(), None)
}

pub(crate) fn rig_with(cards: u8, dark: bool, settings: DeckSettings, saved: Option<&str>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let geometry = settings.geometry(usize::from(cards));
    let panel = geometry.panel();
    let made = Rc::new(RefCell::new(None));
    let out = made.clone();
    let mut s = Surface::open(panel.width as f32, panel.height as f32, move |cx| {
        let deck = cx.new(|_| {
            Deck::new(DeckPolicy {
                delay: None,
                keep_until_saved: false,
            })
        });
        let view = cx.new(|cx| DeckView::new(deck.clone(), settings, cx));
        *out.borrow_mut() = Some((deck, view.clone()));
        Stage {
            content: view.into(),
            dark,
        }
    });
    let (deck, view) = made.borrow_mut().take().unwrap();
    s.app.cx.update(|cx| {
        let mode = if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        es_ui::theme::set_mode(mode, cx);
    });
    let items: Vec<_> = (1..=cards)
        .map(|n| DeckItem {
            saved: saved.map(|name| dir.path().join(name)),
            ..item(dir.path(), n)
        })
        .collect();
    s.app
        .cx
        .update(|cx| deck.update(cx, |d, cx| items.into_iter().for_each(|i| d.show(i, cx))));
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let sub = s.app.cx.update(|cx| {
        cx.subscribe(&deck, move |_, e: &DeckEvent, _| {
            sink.borrow_mut().push(e.clone())
        })
    });
    let watched = view.clone();
    s.app.until("thumbnails load", |cx| {
        (1..=cards).all(|n| watched.read(cx).has_image(Uuid::from_u128(u128::from(n))))
    });
    s.app.cx.advance_clock(es_ui::motion::DECK_ENTER.duration);
    let handle = s.app.window.into();
    s.app
        .cx
        .update_window(handle, |_, window, _| window.refresh())
        .unwrap();
    s.app.settle();
    Rig {
        s,
        deck,
        view,
        events,
        geometry,
        _dir: dir,
        _sub: sub,
    }
}

impl Rig {
    /// Where the view draws card `i` for the cards it holds now.
    pub(crate) fn card(&mut self, i: usize) -> Rect {
        let deck = self.deck.clone();
        let count = self.s.app.cx.update(|cx| deck.read(cx).len());
        DeckSettings::default().geometry(count).card(i)
    }

    /// Points at the newest card and lets the deck spread out.
    pub(crate) fn fan(&mut self) {
        let deck = self.deck.clone();
        let count = self.s.app.cx.update(|cx| deck.read(cx).len());
        let front = self.card(count - 1).center();
        self.point(front.x as f32, front.y as f32);
        self.s
            .app
            .cx
            .advance_clock(es_ui::motion::DECK_FAN.duration);
        self.next_frame();
    }

    pub(crate) fn names(&mut self) -> Vec<String> {
        let deck = self.deck.clone();
        self.s
            .app
            .cx
            .update(|cx| deck.read(cx).items().map(|i| i.name.clone()).collect())
    }

    /// The headless window draws on the next event, so a pointer that changes the card is moved twice before it presses.
    pub(crate) fn point(&mut self, x: f32, y: f32) {
        self.s.hover(x, y, false);
        self.s.hover(x, y, false);
    }

    /// The headless app has no frame clock, so an animation advances only when the window is redrawn.
    pub(crate) fn next_frame(&mut self) {
        let handle = self.s.app.window.into();
        self.s
            .app
            .cx
            .update_window(handle, |_, window, _| window.refresh())
            .unwrap();
        self.s.app.settle();
    }

    pub(crate) fn click(&mut self, x: f32, y: f32) {
        self.point(x, y);
        self.s.down(x, y);
        self.s.up(x, y);
    }

    pub(crate) fn last(&self) -> Option<DeckEvent> {
        self.events.borrow().last().cloned()
    }

    pub(crate) fn requested(&self) -> Vec<(String, DeckAction)> {
        self.events
            .borrow()
            .iter()
            .filter_map(|e| match e {
                DeckEvent::Requested { item, action } => Some((item.name.clone(), *action)),
                DeckEvent::Removed { .. } => None,
            })
            .collect()
    }
}

#[test]
fn hovering_a_card_shows_its_tools_and_holds_the_deck() {
    let mut r = rig(3, true);
    let c = r.card(2);
    r.s.hover((c.x0 + 90.) as f32, (c.y0 + 100.) as f32, false);
    let view = r.view.clone();
    let deck = r.deck.clone();
    r.s.app.cx.update(|cx| {
        assert_eq!(view.read(cx).hovered(), Some(Uuid::from_u128(3)));
        assert!(
            deck.read(cx).hovered(),
            "a pointer over the deck pauses its timers"
        );
    });
    r.s.hover(2., 2., false);
    r.s.app.cx.update(|cx| {
        assert_eq!(view.read(cx).hovered(), None);
        assert!(!deck.read(cx).hovered());
    });
}

#[test]
fn leaving_the_deck_surface_from_a_card_lets_its_timers_run_again() {
    let mut r = rig(3, true);
    let c = r.card(2);
    let (x, y) = ((c.x0 + 90.) as f32, (c.y0 + 100.) as f32);
    r.point(x, y);
    let view = r.view.clone();
    let deck = r.deck.clone();
    r.s.app
        .cx
        .update(|cx| assert_eq!(view.read(cx).hovered(), Some(Uuid::from_u128(3))));
    r.s.send(PlatformInput::MouseExited(MouseExitEvent {
        position: point(px(x), px(y)),
        pressed_button: None,
        modifiers: Modifiers::default(),
    }));
    r.next_frame();
    r.next_frame();
    r.s.app.cx.update(|cx| {
        assert_eq!(
            view.read(cx).hovered(),
            None,
            "the last pointer position is still on the card, but the pointer is on another surface"
        );
        assert!(!deck.read(cx).hovered(), "the dismiss timers run again");
    });

    r.point(x, y);
    r.s.app.cx.update(|cx| {
        assert_eq!(
            view.read(cx).hovered(),
            Some(Uuid::from_u128(3)),
            "coming back onto the same card hovers it again"
        );
    });
}

#[test]
fn the_hover_tools_ask_for_their_actions() {
    let mut r = rig(3, false);
    r.fan();
    let c = r.card(0);
    let mid = c.center();
    r.click((mid.x - 22.) as f32, mid.y as f32);
    assert_eq!(r.requested(), [("shot1".into(), DeckAction::Copy)]);
    assert_eq!(r.names(), ["shot2", "shot3"], "Copy removes the card");

    r.fan();
    let c = r.card(0);
    r.click((c.x0 + 19.) as f32, (c.y0 + 19.) as f32);
    assert_eq!(
        r.requested().last(),
        Some(&("shot2".into(), DeckAction::Pin))
    );

    let c = r.card(0);
    let mid = c.center();
    r.click((mid.x + 22.) as f32, mid.y as f32);
    assert_eq!(
        r.requested().last(),
        Some(&("shot3".into(), DeckAction::Save))
    );
    assert_eq!(r.names(), ["shot3"], "a saving card waits for the save");
}

#[test]
fn an_auto_saved_card_offers_show_in_folder_where_each_preset_puts_save() {
    for preset in [
        LayoutPreset::Standard,
        LayoutPreset::Sharing,
        LayoutPreset::Minimal,
    ] {
        let settings = DeckSettings {
            layout: preset.layout().unwrap(),
            ..DeckSettings::default()
        };
        let mut r = rig_with(1, false, settings, Some("shot1.png"));
        let c = r.card(0);
        let (x, y) = match settings.layout.slot_of(DeckAction::Save).unwrap() {
            Slot::CenterRight => (c.center().x + 22., c.center().y),
            Slot::BottomRight => (c.x1 - 19., c.y1 - 19.),
            slot => panic!("{} puts Save in {slot:?}", preset.name()),
        };
        r.click(x as f32, y as f32);
        assert_eq!(
            r.requested(),
            [("shot1".into(), DeckAction::ShowInFolder)],
            "{}: an auto-saved capture is not saved twice",
            preset.name()
        );
        assert_eq!(r.names(), ["shot1"], "showing the file keeps the card");
    }
}

#[test]
fn dismiss_and_a_click_on_the_picture_close_or_open_the_capture() {
    let mut r = rig(2, false);
    let c = r.card(1);
    r.click((c.x1 - 19.) as f32, (c.y0 + 19.) as f32);
    assert!(matches!(
        r.last(),
        Some(DeckEvent::Removed {
            why: Removal::Dismissed,
            ..
        })
    ));
    assert!(
        r.requested().is_empty(),
        "dismissing asks nothing of the app"
    );

    let c = r.card(0);
    r.click((c.x0 + 90.) as f32, (c.y1 - 12.) as f32);
    assert_eq!(r.requested(), [("shot1".into(), DeckAction::Edit)]);
    assert!(r.names().is_empty());
}

#[test]
fn the_header_saves_or_clears_every_card() {
    let mut r = rig(3, false);
    r.fan();
    let header = r.geometry.header().unwrap();
    r.click((header.x1 - 20.) as f32, header.center().y as f32);
    assert!(r.names().is_empty(), "Clear All empties the deck");
    let cleared = r
        .events
        .borrow()
        .iter()
        .filter(|e| {
            matches!(
                e,
                DeckEvent::Removed {
                    why: Removal::Cleared,
                    ..
                }
            )
        })
        .count();
    assert_eq!(cleared, 3);

    let mut r = rig(2, false);
    r.fan();
    let header = r.geometry.header().unwrap();
    r.click((header.x1 - 75.) as f32, header.center().y as f32);
    assert_eq!(
        r.requested(),
        [
            ("shot1".into(), DeckAction::Save),
            ("shot2".into(), DeckAction::Save)
        ]
    );
}

#[test]
fn pictures_of_the_deck_at_rest_and_hovered() {
    for dark in [false, true] {
        let theme = if dark { "dark" } else { "light" };
        let mut r = rig(3, dark);
        r.s.app.screenshot(&format!("deck-entry-{theme}"));
        let c = r.card(2);
        let (x, y) = ((c.x0 + 150.) as f32, (c.y0 + 100.) as f32);
        let rest = pixel(&r.s.app.screenshot("deck-probe"), x, y);
        r.point(x, y);
        r.s.app.cx.advance_clock(es_ui::motion::DECK_HOVER.duration);
        r.next_frame();
        let hovered = r.s.app.screenshot(&format!("deck-hover-{theme}"));
        let scrim = pixel(&hovered, x, y);
        assert!(
            (0..3).all(|i| u16::from(scrim[i]) + 40 < u16::from(rest[i])),
            "the hover scrim darkens the picture: {rest:?} -> {scrim:?}"
        );
    }
}
