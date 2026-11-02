use super::card::{Card, black, white};
use super::layout::{CardSize, Corner, DEFAULT_MARGIN, DeckGeometry, ToolLayout};
use crate::capture::strings::fl;
use es_app::deck::{Deck, DeckAction};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SurfaceHandle as _, SurfaceRole,
};
use es_geom::{Point, Rect};
use es_model::config::Overlay;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::theme::UI_FONT;
use gpui::{
    App, AppContext as _, Context, DispatchPhase, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, MouseExitEvent, MouseMoveEvent, ParentElement as _,
    Render, RenderImage, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, canvas, div, prelude::FluentBuilder as _, px,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeckSettings {
    pub size: CardSize,
    pub corner: Corner,
    pub margin: f64,
    pub layout: ToolLayout,
    pub always_show_actions: bool,
    /// Cloud Share is drawn only when sharing is set up.
    pub share: bool,
}

impl Default for DeckSettings {
    fn default() -> Self {
        DeckSettings {
            size: CardSize::Small,
            corner: Corner::BottomRight,
            margin: DEFAULT_MARGIN,
            layout: ToolLayout::standard(),
            always_show_actions: false,
            share: false,
        }
    }
}

impl DeckSettings {
    pub fn from_settings(overlay: &Overlay) -> Self {
        DeckSettings {
            size: CardSize::parse(&overlay.card_size),
            corner: Corner::parse(&overlay.position),
            margin: f64::from(overlay.edge_margin),
            layout: ToolLayout::parse(&overlay.layout),
            always_show_actions: overlay.always_show_actions,
            share: false,
        }
    }

    pub fn geometry(&self, count: usize) -> DeckGeometry {
        DeckGeometry::new(self.size.size(), self.margin, self.corner, count)
    }
}

/// GPUI images are straight-alpha BGRA.
pub fn load_thumbnail(path: &Path) -> Option<Arc<RenderImage>> {
    let mut rgba = image::open(path).ok()?.into_rgba8();
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new([image::Frame::new(rgba)])))
}

pub struct DeckView {
    deck: Entity<Deck>,
    settings: DeckSettings,
    images: HashMap<Uuid, Option<Arc<RenderImage>>>,
    focus: HashMap<Uuid, FocusHandle>,
    hovered: Option<Uuid>,
    cards: Vec<(Uuid, Rect)>,
    showings: usize,
    focused: Option<Uuid>,
    _observe: Subscription,
}

impl DeckView {
    pub fn new(deck: Entity<Deck>, settings: DeckSettings, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&deck, |view, _, cx| view.sync(cx));
        let mut view = DeckView {
            deck,
            settings,
            images: HashMap::new(),
            focus: HashMap::new(),
            hovered: None,
            cards: Vec::new(),
            showings: 0,
            focused: None,
            _observe: observe,
        };
        view.sync(cx);
        view
    }

    pub fn hovered(&self) -> Option<Uuid> {
        self.hovered
    }

    pub fn has_image(&self, id: Uuid) -> bool {
        self.images.get(&id).is_some_and(Option::is_some)
    }

    fn sync(&mut self, cx: &mut Context<Self>) {
        let items: Vec<(Uuid, std::path::PathBuf)> = self
            .deck
            .read(cx)
            .items()
            .map(|i| (i.id, i.thumb.clone()))
            .collect();
        self.images
            .retain(|id, _| items.iter().any(|(i, _)| i == id));
        self.focus
            .retain(|id, _| items.iter().any(|(i, _)| i == id));
        if self
            .hovered
            .is_some_and(|h| !items.iter().any(|(i, _)| *i == h))
        {
            self.hovered = None;
        }
        for (id, thumb) in items {
            if self.images.contains_key(&id) {
                continue;
            }
            self.images.insert(id, None);
            let load = cx.background_spawn(async move { load_thumbnail(&thumb) });
            cx.spawn(async move |view, cx| {
                let image = load.await;
                let _ = view.update(cx, |view, cx| {
                    if let Some(slot) = view.images.get_mut(&id) {
                        *slot = image;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Element hover keeps the last position after the pointer leaves, so it would stay on the card.
    fn pointer_at(&mut self, at: Option<Point>, cx: &mut Context<Self>) {
        let now = at.and_then(|p| {
            self.cards
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(id, _)| *id)
        });
        if now != self.hovered {
            if now.is_some() {
                self.showings += 1;
            }
            self.hovered = now;
            let deck = self.deck.clone();
            cx.defer(move |cx| deck.update(cx, |d, cx| d.set_hovered(now.is_some(), cx)));
            cx.notify();
        }
    }

    fn pill(
        &self,
        id: &'static str,
        label: String,
        action: fn(&mut Deck, &mut Context<Deck>),
    ) -> impl IntoElement {
        let deck = self.deck.clone();
        div()
            .id(id)
            .accessible(Role::Button, label.clone())
            .cursor_pointer()
            .px(px(8.))
            .py(px(3.))
            .rounded_full()
            .bg(black(0.55))
            .font_family(UI_FONT)
            .text_size(px(10.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(white(0.9))
            .child(label)
            .on_click(move |_, _, cx| deck.update(cx, action))
    }
}

impl Render for DeckView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let deck = self.deck.read(cx);
        let items: Vec<_> = deck.items().cloned().collect();
        let can_save_all = deck.can_save_all();
        let geometry = self.settings.geometry(items.len());
        for item in &items {
            self.focus
                .entry(item.id)
                .or_insert_with(|| cx.focus_handle());
        }
        let focused = items
            .iter()
            .find(|i| {
                self.focus
                    .get(&i.id)
                    .is_some_and(|f| f.contains_focused(window, cx))
            })
            .map(|i| i.id);
        if focused != self.focused {
            self.focused = focused;
            let deck = self.deck.clone();
            cx.defer(move |cx| deck.update(cx, |d, cx| d.set_focused(focused, cx)));
        }
        let header = geometry.header().map(|r| {
            let row = div()
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .w(px(r.width() as f32))
                .h(px(r.height() as f32))
                .flex()
                .items_center()
                .gap(px(6.))
                .when(self.settings.corner == Corner::BottomRight, |row| {
                    row.justify_end()
                });
            row.when(can_save_all, |row| {
                row.child(self.pill("deck-save-all", fl!("deck-save-all"), Deck::save_all))
            })
            .child(self.pill(
                "deck-clear-all",
                fl!("deck-clear-all"),
                Deck::clear_all,
            ))
        });
        self.cards = items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id, geometry.card(i)))
            .collect();
        let view = cx.entity().downgrade();
        let pointer = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moved = view.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        let at = Point::new(f64::from(e.position.x), f64::from(e.position.y));
                        let _ = moved.update(cx, |v, cx| v.pointer_at(Some(at), cx));
                    }
                });
                let left = view.clone();
                window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        let _ = left.update(cx, |v, cx| v.pointer_at(None, cx));
                    }
                });
            },
        )
        .absolute()
        .size_full();
        let cards = items.into_iter().enumerate().map(|(i, item)| {
            let r: Rect = geometry.card(i);
            let id = item.id;
            let focus = self
                .focus
                .get(&id)
                .cloned()
                .unwrap_or_else(|| cx.focus_handle());
            let show = self.settings.always_show_actions
                || self.hovered == Some(id)
                || self.focused == Some(id);
            let deck = self.deck.clone();
            let on_action = Rc::new(move |action: DeckAction, _: &mut Window, cx: &mut App| {
                deck.update(cx, |d, cx| d.act(id, action, cx));
            });
            let image = self.images.get(&id).cloned().flatten();
            div()
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .child(
                    Card::new(
                        item,
                        geometry.card,
                        image,
                        self.settings.layout,
                        self.settings.size.control_scale(),
                        focus,
                        on_action,
                    )
                    .tools(show.then_some(self.showings))
                    .share(self.settings.share),
                )
        });
        div()
            .id("deck")
            .accessible(Role::Group, SharedString::from(fl!("deck-window-title")))
            .relative()
            .size_full()
            .child(pointer)
            .children(header)
            .children(cards)
    }
}

struct Shown {
    handle: LayerSurfaceHandle,
    count: usize,
}

/// Keeps a deck surface mapped while the deck has cards, sized to them, with only the cards taking input.
pub struct DeckSurface {
    surfaces: Rc<LayerShellSurfaces>,
    deck: Entity<Deck>,
    settings: DeckSettings,
    shown: RefCell<Option<Shown>>,
    observe: RefCell<Option<Subscription>>,
}

impl DeckSurface {
    pub fn attach(
        surfaces: Rc<LayerShellSurfaces>,
        deck: &Entity<Deck>,
        settings: DeckSettings,
        cx: &mut App,
    ) -> Rc<Self> {
        let this = Rc::new(DeckSurface {
            surfaces,
            deck: deck.clone(),
            settings,
            shown: RefCell::default(),
            observe: RefCell::default(),
        });
        let weak: Weak<Self> = Rc::downgrade(&this);
        let observe = cx.observe(deck, move |_, cx| {
            if let Some(this) = weak.upgrade()
                && let Err(error) = this.sync(cx)
            {
                tracing::warn!(%error, "deck surface");
            }
        });
        *this.observe.borrow_mut() = Some(observe);
        if let Err(error) = this.sync(cx) {
            tracing::warn!(%error, "deck surface");
        }
        this
    }

    pub fn is_shown(&self) -> bool {
        self.shown.borrow().is_some()
    }

    fn sync(&self, cx: &mut App) -> Result<()> {
        let deck = self.deck.read(cx);
        let count = if deck.is_hidden() { 0 } else { deck.len() };
        if count == 0 {
            if let Some(shown) = self.shown.borrow_mut().take() {
                shown.handle.close(cx);
            }
            return Ok(());
        }
        let geometry = self.settings.geometry(count);
        let mut slot = self.shown.borrow_mut();
        match slot.as_mut() {
            Some(shown) if shown.count == count => return Ok(()),
            Some(shown) => {
                shown.count = count;
                shown.handle.resize(geometry.panel(), cx);
            }
            None => {
                let output = self
                    .surfaces
                    .test_output()
                    .cloned()
                    .or_else(|| self.surfaces.source().pointer_output())
                    .or_else(|| self.surfaces.source().focused_output())
                    .ok_or_else(|| Error::NotFound("an output for the deck".into()))?;
                let handle =
                    self.surfaces
                        .open_surface(SurfaceRole::Deck, &output, geometry.panel(), cx)?;
                let (deck, settings) = (self.deck.clone(), self.settings);
                let view = cx.new(|cx| DeckView::new(deck, settings, cx));
                handle.set_content(view.into(), cx);
                *slot = Some(Shown { handle, count });
            }
        }
        if let Some(shown) = slot.as_ref() {
            let output = shown.handle.output_info().logical;
            shown.handle.set_position(geometry.origin(output), cx);
            shown.handle.set_input_region(&geometry.input_region(), cx);
        }
        Ok(())
    }
}
