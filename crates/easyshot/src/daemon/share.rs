use super::windows::Wiring;
use es_app::deck::{Deck, DeckItem};
use es_app::history::History;
use es_app::platform_ui::{ClipboardSink, LayerShellSurfaces, SharePopoverSurface};
use es_app::settings_model::SettingsModel;
use es_app::share::{DeckShare, LiveTargets, PopoverHost, ShareHooks, Shares, SystemKeyring};
use es_overlays::deck::view::DeckSurface;
use es_protocol::{Route, SettingsPage};
use es_store::Paths;
use gpui::{App, AppContext as _, Entity, Global};
use std::rc::Rc;
use std::sync::Arc;

pub fn engine(
    paths: &Paths,
    settings: &Entity<SettingsModel>,
    history: &Entity<History>,
    clipboard: Rc<dyn ClipboardSink>,
    cx: &mut App,
) -> Entity<Shares> {
    let targets = LiveTargets::new(
        paths.clone(),
        env!("EASYSHOT_VERSION"),
        Arc::new(SystemKeyring),
    );
    let (settings, history) = (settings.clone(), history.clone());
    cx.new(|cx| Shares::new(paths, settings, history, clipboard, Arc::new(targets), cx))
}

type Open = Rc<dyn Fn(&DeckItem, &mut App)>;

struct DeckShares(Open);

impl Global for DeckShares {}

/// Share on a deck card opens the popover at that card's Share button.
pub(super) fn install(
    shares: Entity<Shares>,
    deck: &Entity<Deck>,
    deck_surface: Rc<DeckSurface>,
    surfaces: Rc<LayerShellSurfaces>,
    wiring: Rc<Wiring>,
    clipboard: Rc<dyn ClipboardSink>,
    cx: &mut App,
) {
    let host: Rc<dyn PopoverHost> = Rc::new(SharePopoverSurface::new(surfaces));
    let hooks = ShareHooks {
        open_settings: Rc::new(move |cx| {
            wiring.dispatch(Route::Settings(Some(SettingsPage::Sharing)), cx);
        }),
        open_url: Rc::new(|url, cx| cx.open_url(url)),
    };
    let deck = deck.clone();
    let flow = cx.new(|cx| DeckShare::new(shares, deck, host, hooks, clipboard, cx));
    cx.set_global(DeckShares(Rc::new(move |item, cx| {
        let anchor = deck_surface.share_anchor(item.id, cx);
        let item = item.clone();
        flow.update(cx, |f, cx| f.open(item, anchor, cx));
    })));
}

pub(super) fn open_from_deck(item: &DeckItem, cx: &mut App) {
    let Some(open) = cx.try_global::<DeckShares>().map(|d| d.0.clone()) else {
        tracing::warn!("share from the deck dropped: none installed");
        return;
    };
    open(item, cx);
}

/// Records each card asked for instead of opening a popover.
#[cfg(test)]
pub(super) fn record(cx: &mut App) -> Rc<std::cell::RefCell<Vec<uuid::Uuid>>> {
    let asked: Rc<std::cell::RefCell<Vec<uuid::Uuid>>> = Rc::default();
    let into = asked.clone();
    cx.set_global(DeckShares(Rc::new(move |item, _| {
        into.borrow_mut().push(item.id)
    })));
    asked
}
