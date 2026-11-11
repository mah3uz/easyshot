#![allow(clippy::unwrap_used)]
use crate::deck::item;
use es_app::deck::{Deck, DeckAction, DeckPolicy};
use es_app::platform_ui::SharePopoverPanel;
use es_geom::{Point, Rect, Size};
use es_overlays::deck::card::button_rect;
use es_overlays::deck::view::{DeckSettings, DeckView};
use es_testkit::Headless;
use es_ui::components::{SharePopover, ShareView};
use es_ui::tokens::ThemeMode;
use gpui::{
    AnyView, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px, rgb,
};
use gpui_component::Root;
use std::rc::Rc;

const W: f32 = 520.;
const H: f32 = 470.;

struct Desk {
    deck: AnyView,
    panel: Option<Entity<SharePopoverPanel>>,
    at: Point,
    size: (f32, f32),
    dark: bool,
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let desk = if self.dark {
            rgb(0x2a2d33)
        } else {
            rgb(0xe9ecf1)
        };
        div()
            .size_full()
            .bg(desk)
            .child(
                div()
                    .absolute()
                    .left(px(self.at.x as f32))
                    .top(px(self.at.y as f32))
                    .w(px(self.size.0))
                    .h(px(self.size.1))
                    .child(self.deck.clone()),
            )
            .children(
                self.panel
                    .clone()
                    .map(|p| div().absolute().size_full().child(p)),
            )
    }
}

/// One card in the screen's corner, with the share popover open at its Share button or not.
fn picture(dark: bool, popover: bool, shared: bool, name: &str) -> image::RgbaImage {
    let dir = tempfile::tempdir().unwrap();
    let settings = DeckSettings {
        share: true,
        ..DeckSettings::default()
    };
    let geometry = settings.geometry(1);
    let room = geometry.panel();
    let at = Point::new(f64::from(W) - room.width, f64::from(H) - room.height);
    let card = item(dir.path(), 1);
    let id = card.id;
    let mut deck_entity = None;
    let mut view_entity = None;
    let mut app = Headless::open(es_ui::assets::Assets, es_ui::init, W, H, |window, cx| {
        es_ui::theme::set_mode(
            if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            cx,
        );
        let deck = cx.new(|_| {
            Deck::new(DeckPolicy {
                delay: None,
                keep_until_saved: false,
            })
        });
        let view = cx.new(|cx| DeckView::new(deck.clone(), settings, cx));
        let c = geometry.card(0);
        let b = button_rect(
            settings.layout.slot_of(DeckAction::Share),
            c.size(),
            settings.size.control_scale(),
        );
        let trigger = Rect::from_origin_size(
            Point::new(at.x + c.x0 + b.x0, at.y + c.y0),
            Size::new(b.width(), 1.),
        );
        let panel = popover.then(|| {
            cx.new(|_| {
                SharePopoverPanel::new(
                    trigger,
                    f64::from(W),
                    Box::new(|window, cx| {
                        cx.new(|cx| {
                            let mut p = SharePopover::new(window, cx);
                            p.set_view(ShareView::Ready, cx);
                            p
                        })
                    }),
                    Rc::new(|_| {}),
                )
            })
        });
        deck_entity = Some(deck);
        view_entity = Some(view.clone());
        let desk = cx.new(|_| Desk {
            deck: view.into(),
            panel,
            at,
            size: (room.width as f32, room.height as f32),
            dark,
        });
        cx.new(|cx| Root::new(desk, window, cx))
    });
    let (deck, view) = (deck_entity.unwrap(), view_entity.unwrap());
    app.cx.update(|cx| {
        deck.update(cx, |d, cx| {
            d.show(card, cx);
            if popover {
                d.hold(Some(id), cx);
            }
            d.set_shared(id, shared, cx);
        })
    });
    app.until("the thumbnail", |cx| view.read(cx).has_image(id));
    for _ in 0..3 {
        app.cx.advance_clock(es_ui::motion::DECK_ENTER.duration);
        app.cx.advance_clock(es_ui::motion::POPOVER_OPEN.duration);
        std::thread::sleep(es_ui::motion::POPOVER_OPEN.duration);
        let handle = app.window.into();
        app.cx
            .update_window(handle, |_, window, _| window.refresh())
            .unwrap();
        app.settle();
    }
    let image = app.screenshot(name);
    assert!(app.cx.update(|cx| deck.read(cx).is_shared(id)) == shared);
    image
}

#[test]
fn pictures_of_a_card_with_its_share_popover_and_with_its_shared_mark() {
    for (dark, tag) in [(false, "light"), (true, "dark")] {
        let open = picture(dark, true, false, &format!("deck-share-popover-{tag}"));
        let plain = picture(dark, false, false, &format!("deck-share-plain-{tag}"));
        let marked = picture(dark, false, true, &format!("deck-share-mark-{tag}"));
        assert!(open != plain, "{tag}: the popover is drawn over the desk");
        assert!(marked != plain, "{tag}: a shared card carries its mark");
    }
}
