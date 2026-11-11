use super::flow::{Anchor, PopoverHost, ShareOrigin};
use super::{Shares, SharesChanged};
use crate::deck::{Deck, DeckItem};
use gpui::{App, Context, Entity, Subscription};
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use uuid::Uuid;

/// Share on a deck card: the popover points at the card's Share button while the card is there.
pub struct DeckOrigin {
    pub item: DeckItem,
    pub deck: Entity<Deck>,
    pub host: Rc<dyn PopoverHost>,
    pub anchor: Option<Anchor>,
}

impl ShareOrigin for DeckOrigin {
    fn subject(&self) -> &Path {
        &self.item.file
    }

    fn host(&self) -> Rc<dyn PopoverHost> {
        self.host.clone()
    }

    fn anchor(&self, cx: &App) -> Option<Anchor> {
        self.deck
            .read(cx)
            .get(self.item.id)
            .is_some()
            .then(|| self.anchor.clone())
            .flatten()
    }

    fn hold(&self, held: bool, cx: &mut App) {
        let id = held.then_some(self.item.id);
        self.deck.update(cx, |d, cx| d.hold(id, cx));
    }
}

/// The cloud mark on deck cards with a live link.
pub struct DeckMarks {
    shares: Entity<Shares>,
    deck: Entity<Deck>,
    marked: HashSet<Uuid>,
    _subscriptions: [Subscription; 2],
}

impl DeckMarks {
    pub fn new(shares: Entity<Shares>, deck: Entity<Deck>, cx: &mut Context<Self>) -> Self {
        let subscriptions = [
            cx.subscribe(&shares, |this, _, _: &SharesChanged, cx| {
                this.mark(true, cx)
            }),
            cx.observe(&deck, |this, _, cx| this.mark(false, cx)),
        ];
        Self {
            shares,
            deck,
            marked: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }

    /// Only a change to which cards are here, or to the shares, asks the engine again.
    fn mark(&mut self, shares_changed: bool, cx: &mut Context<Self>) {
        let items: Vec<DeckItem> = self.deck.read(cx).items().cloned().collect();
        let ids: HashSet<Uuid> = items.iter().map(|i| i.id).collect();
        if !shares_changed && ids == self.marked {
            return;
        }
        self.marked = ids;
        let shares = self.shares.read(cx);
        let marks: Vec<(Uuid, bool)> = items
            .iter()
            .map(|i| (i.id, shares.for_file(&i.file, cx).is_some()))
            .collect();
        self.deck.update(cx, |d, cx| {
            for (id, shared) in marks {
                d.set_shared(id, shared, cx);
            }
        });
    }
}
