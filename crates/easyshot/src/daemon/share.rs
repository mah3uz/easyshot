use super::windows::Wiring;
use es_app::deck::{Deck, DeckItem};
use es_app::history::History;
use es_app::platform_ui::{ClipboardSink, LayerShellSurfaces, SharePopoverSurface};
use es_app::settings_model::SettingsModel;
use es_app::share::{
    DeckMarks, DeckOrigin, LiveTargets, PopoverHost, ShareFlow, ShareHooks, ShareOrigin, Shares,
    SystemKeyring,
};
use es_app::share::{FileOrigin, WindowPopover};
use es_editor::{EditorWindow, ShareRequested};
use es_overlays::deck::view::DeckSurface;
use es_protocol::{Route, SettingsPage};
use es_store::Paths;
use gpui::{App, AppContext as _, Entity, Global, Task, WeakEntity};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

/// The share service compares this with its minimum, so it must be the bare `x.y.z`, not the shown `v0.1.0+hash`.
const SHARE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn engine(
    paths: &Paths,
    settings: &Entity<SettingsModel>,
    history: &Entity<History>,
    clipboard: Rc<dyn ClipboardSink>,
    cx: &mut App,
) -> Entity<Shares> {
    let targets = LiveTargets::new(paths.clone(), SHARE_VERSION, Arc::new(SystemKeyring));
    let (settings, history) = (settings.clone(), history.clone());
    cx.new(|cx| Shares::new(paths, settings, history, clipboard, Arc::new(targets), cx))
}

type Open = Rc<dyn Fn(&DeckItem, &mut App)>;

struct DeckShares(Open);

impl Global for DeckShares {}

struct Flow {
    flow: Entity<ShareFlow>,
    _marks: Entity<DeckMarks>,
}

impl Global for Flow {}

/// Opens the share popover for a surface; nothing happens before sharing is installed.
pub(super) fn open(origin: Rc<dyn ShareOrigin>, cx: &mut App) {
    match cx.try_global::<Flow>().map(|f| f.flow.clone()) {
        Some(flow) => flow.update(cx, |f, cx| f.open(origin, cx)),
        None => tracing::warn!("share dropped: none installed"),
    }
}

/// Share in the editor sends the image as edited, recorded against the capture it opened.
struct EditorOrigin {
    file: PathBuf,
    editor: WeakEntity<EditorWindow>,
    host: WindowPopover,
}

impl ShareOrigin for EditorOrigin {
    fn subject(&self) -> &Path {
        &self.file
    }

    fn host(&self) -> Rc<dyn PopoverHost> {
        Rc::new(self.host.clone())
    }

    fn file(&self, cx: &mut App) -> Task<Result<PathBuf, String>> {
        match self.editor.upgrade() {
            Some(editor) => editor.update(cx, |e, cx| e.share_file(cx)),
            None => Task::ready(Err(String::new())),
        }
    }
}

pub(super) fn editor(editor: &Entity<EditorWindow>, file: &Path, cx: &mut App) {
    let host = WindowPopover::default();
    editor.update(cx, |e, cx| e.set_share(host.clone(), cx));
    let file = file.to_owned();
    cx.subscribe(editor, move |editor, _: &ShareRequested, cx| {
        let origin = EditorOrigin {
            file: file.clone(),
            editor: editor.downgrade(),
            host: host.clone(),
        };
        open(Rc::new(origin), cx);
    })
    .detach();
}

/// Share in the trim viewer sends the clip as trimmed and toned there, recorded against the recording.
#[cfg(feature = "player")]
struct PlayerOrigin {
    file: FileOrigin,
    player: WeakEntity<es_overlays::player::PlayerWindow>,
}

#[cfg(feature = "player")]
impl ShareOrigin for PlayerOrigin {
    fn subject(&self) -> &Path {
        self.file.subject()
    }

    fn host(&self) -> Rc<dyn PopoverHost> {
        self.file.host()
    }

    fn export(&self, cx: &App) -> Option<es_record::clip::export::ExportJob> {
        Some(self.player.upgrade()?.read(cx).share_job())
    }
}

#[cfg(feature = "player")]
pub(super) fn player(
    player: &Entity<es_overlays::player::PlayerWindow>,
    video: &Path,
    cx: &mut App,
) {
    let host = WindowPopover::default();
    player.update(cx, |p, cx| p.set_share(host.clone(), cx));
    let file = video.to_owned();
    cx.subscribe(
        player,
        move |player, _: &es_overlays::player::ShareRequested, cx| {
            let origin = PlayerOrigin {
                file: FileOrigin {
                    file: file.clone(),
                    host: host.clone(),
                },
                player: player.downgrade(),
            };
            open(Rc::new(origin), cx);
        },
    )
    .detach();
}

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
    let marks = cx.new(|cx| DeckMarks::new(shares.clone(), deck.clone(), cx));
    let flow = cx.new(|cx| ShareFlow::new(shares, hooks, clipboard, cx));
    cx.set_global(Flow {
        flow: flow.clone(),
        _marks: marks,
    });
    cx.set_global(DeckShares(Rc::new(move |item, cx| {
        let origin = DeckOrigin {
            item: item.clone(),
            deck: deck.clone(),
            host: host.clone(),
            anchor: deck_surface.share_anchor(item.id, cx),
        };
        flow.update(cx, |f, cx| f.open(Rc::new(origin), cx));
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

#[cfg(test)]
mod version_tests {
    #[test]
    fn the_share_service_gets_a_bare_version_it_can_compare() {
        let parts: Vec<&str> = super::SHARE_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{}", super::SHARE_VERSION);
        assert!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "the service refuses anything but x.y.z as too old: {}",
            super::SHARE_VERSION
        );
    }
}
