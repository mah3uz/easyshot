use super::card::{Card, RADIUS, black, card_shadow, white};
use super::layout::{CardSize, Corner, DEFAULT_MARGIN, DeckGeometry, PEEKING, ToolLayout};
use crate::capture::strings::fl;
use es_app::deck::{Deck, DeckAction};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SurfaceHandle as _, SurfaceRole,
};
use es_geom::{Point, Rect};
use es_model::config::{CardActions, Overlay};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::motion::{
    Clocked, DECK_ENTER, DECK_ENTER_REDUCED, DECK_ENTER_RISE, DECK_ENTER_SCALE, DECK_FAN, tween,
};
use es_ui::theme::UI_FONT;
use gpui::{
    App, AppContext as _, Context, DispatchPhase, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, MouseExitEvent, MouseMoveEvent, ParentElement as _,
    Render, RenderImage, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription,
    Window, canvas, div, prelude::FluentBuilder as _, px,
};
use std::cell::{Cell, RefCell};
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
    /// Whether Cloud Share is drawn.
    pub share: bool,
    pub actions: CardActions,
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
            actions: CardActions::default(),
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
            share: true,
            actions: overlay.actions.or_default(),
        }
    }

    pub fn geometry(&self, count: usize) -> DeckGeometry {
        DeckGeometry::new(self.size.size(), self.margin, self.corner, count)
    }
}

/// GPUI images are straight-alpha BGRA.
pub fn load_thumbnail(path: &Path) -> Option<Arc<RenderImage>> {
    let mut rgba = es_store::decode::open_image(path).ok()?.into_rgba8();
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
    /// When each card arrived, and whether it grows in or only fades in under Reduce Motion.
    arrivals: HashMap<Uuid, Option<(Clocked, bool)>>,
    inside: bool,
    fan: f32,
    cards: Vec<(Uuid, Rect)>,
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
            arrivals: HashMap::new(),
            inside: false,
            fan: 0.,
            cards: Vec::new(),
            focused: None,
            _observe: observe,
        };
        view.sync(cx);
        view
    }

    /// Cards already on screen when the deck is rebuilt keep their place instead of entering again.
    pub fn settled(mut self) -> Self {
        self.arrivals.values_mut().for_each(|a| *a = None);
        self
    }

    pub fn hovered(&self) -> Option<Uuid> {
        self.hovered
    }

    pub fn fanned(&self) -> bool {
        self.inside || self.focused.is_some()
    }

    /// How far the deck was drawn spread out: 0 folded, 1 fanned.
    pub fn spread(&self) -> f32 {
        self.fan
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
        self.arrivals
            .retain(|id, _| items.iter().any(|(i, _)| i == id));
        if self
            .hovered
            .is_some_and(|h| !items.iter().any(|(i, _)| *i == h))
        {
            self.hover(false, None, cx);
        }
        for (id, thumb) in items {
            if self.images.contains_key(&id) {
                continue;
            }
            self.images.insert(id, None);
            let arrival = Clocked::start(DECK_ENTER, cx)
                .map(|c| (c, true))
                .or_else(|| Clocked::start(DECK_ENTER_REDUCED, cx).map(|c| (c, false)));
            self.arrivals.insert(id, arrival);
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
    /// The pointer is tested against the region taking it now, so moving from the stack onto the
    /// fanned cards, or across the gaps between them, keeps the deck fanned.
    fn pointer_at(&mut self, at: Option<Point>, cx: &mut Context<Self>) {
        let region = self
            .settings
            .geometry(self.cards.len())
            .input_region(self.fanned());
        let inside = at.is_some_and(|p| region.iter().any(|r| r.contains(p)));
        let card = at.filter(|_| inside).and_then(|p| {
            self.cards
                .iter()
                .find(|(_, r)| r.contains(p))
                .map(|(id, _)| *id)
        });
        self.hover(inside, card, cx);
    }

    fn hover(&mut self, inside: bool, card: Option<Uuid>, cx: &mut Context<Self>) {
        if inside != self.inside {
            self.inside = inside;
            let deck = self.deck.clone();
            cx.defer(move |cx| deck.update(cx, |d, cx| d.set_hovered(inside, cx)));
            cx.notify();
        }
        if card != self.hovered {
            self.hovered = card;
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
        // A click focuses the card too, and the deck keeps keyboard focus after the pointer leaves.
        let keyboard = window.last_input_was_keyboard();
        let focused = items
            .iter()
            .find(|i| {
                keyboard
                    && self
                        .focus
                        .get(&i.id)
                        .is_some_and(|f| f.contains_focused(window, cx))
            })
            .map(|i| i.id);
        if focused != self.focused {
            self.focused = focused;
            let deck = self.deck.clone();
            cx.defer(move |cx| deck.update(cx, |d, cx| d.set_focused(focused, cx)));
        }
        let fanned = self.fanned();
        let fan = tween(
            "deck-fan",
            if fanned { 1. } else { 0. },
            DECK_FAN,
            window,
            cx,
        );
        self.fan = fan;
        let header = geometry.header().filter(|_| fan > 0.).map(|r| {
            let row = div()
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .w(px(r.width() as f32))
                .h(px(r.height() as f32))
                .flex()
                .items_center()
                .gap(px(6.))
                .opacity(fan.clamp(0., 1.))
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
        let count = items.len();
        let badge = geometry.badge().filter(|_| fan < 1.).map(|r| {
            div()
                .id("deck-count")
                .accessible(Role::Status, fl!("deck-stack-a11y", count = count))
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .size(px(r.width() as f32))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(white(0.95))
                .shadow(card_shadow())
                .font_family(UI_FONT)
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(black(0.85))
                .opacity((1. - fan).clamp(0., 1.))
                .child(fl!("deck-stack-count", count = count))
        });
        let cards = items.into_iter().enumerate().map(|(i, item)| {
            let (grow, arrived) = match self.arrivals.get(&item.id).copied().flatten() {
                Some((clock, moves)) => {
                    let e = clock.progress(window, cx);
                    (if moves { e } else { 1. }, (e * 1.5).clamp(0., 1.))
                }
                None => (1., 1.),
            };
            let r = entering(geometry.at(i, f64::from(fan)), grow);
            let depth = geometry.depth(i);
            let dim = [0., 0.22, 0.4][depth.min(PEEKING)] * (1. - fan).clamp(0., 1.);
            let id = item.id;
            let focus = self
                .focus
                .get(&id)
                .cloned()
                .unwrap_or_else(|| cx.focus_handle());
            let show = (self.settings.always_show_actions
                || self.hovered == Some(id)
                || self.focused == Some(id))
                && (depth == 0 || fanned);
            let deck = self.deck.clone();
            let on_action = Rc::new(move |action: DeckAction, _: &mut Window, cx: &mut App| {
                deck.update(cx, |d, cx| d.act(id, action, cx));
            });
            let image = self.images.get(&id).cloned().flatten();
            div()
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .w(px(r.width() as f32))
                .h(px(r.height() as f32))
                .when(depth > PEEKING, |card| card.opacity(fan.clamp(0., 1.)))
                .when(arrived < 1., |card| card.opacity(arrived))
                .child(
                    Card::new(
                        item,
                        r.size(),
                        image,
                        self.settings.layout,
                        self.settings.size.control_scale(),
                        focus,
                        on_action,
                    )
                    .tools(show)
                    .share(self.settings.share)
                    .actions(self.settings.actions),
                )
                .when(dim > 0., |card| {
                    card.child(div().absolute().inset_0().rounded(RADIUS).bg(black(dim)))
                })
        });
        div()
            .id("deck")
            .accessible(Role::Group, SharedString::from(fl!("deck-window-title")))
            .relative()
            .size_full()
            .child(pointer)
            .children(header)
            .children(cards)
            .children(badge)
    }
}

/// `r` grown from its entrance scale and risen into place, `grow` of the way along.
fn entering(r: Rect, grow: f32) -> Rect {
    if grow == 1. {
        return r;
    }
    let left = f64::from(1. - grow);
    let scale = 1. - left * f64::from(1. - DECK_ENTER_SCALE);
    let size = es_geom::Size::new(r.width() * scale, r.height() * scale);
    let centre = r.center();
    let below = Point::new(centre.x, centre.y + left * f64::from(DECK_ENTER_RISE));
    Rect::from_center_size(below, size)
}

struct Shown {
    handle: LayerSurfaceHandle,
    count: usize,
    fanned: bool,
}

/// Keeps a deck surface mapped while the deck has cards, sized to them fanned out, with only the
/// stack or the fanned cards taking input; fanning never resizes it, so the pointer stays put.
pub struct DeckSurface {
    surfaces: Rc<LayerShellSurfaces>,
    deck: Entity<Deck>,
    settings: Cell<DeckSettings>,
    shown: RefCell<Option<Shown>>,
    observe: RefCell<Option<Subscription>>,
    rebuilt: Cell<bool>,
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
            settings: Cell::new(settings),
            shown: RefCell::default(),
            observe: RefCell::default(),
            rebuilt: Cell::default(),
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

    pub fn settings(&self) -> DeckSettings {
        self.settings.get()
    }

    /// A mapped deck is rebuilt, since its view keeps the settings it was opened with.
    pub fn set_settings(&self, settings: DeckSettings, cx: &mut App) {
        if self.settings.get() == settings {
            return;
        }
        self.settings.set(settings);
        if let Some(shown) = self.shown.borrow_mut().take() {
            shown.handle.close(cx);
            self.rebuilt.set(true);
        }
        if let Err(error) = self.sync(cx) {
            tracing::warn!(%error, "deck surface");
        }
    }

    pub fn is_shown(&self) -> bool {
        self.shown.borrow().is_some()
    }

    fn sync(&self, cx: &mut App) -> Result<()> {
        let deck = self.deck.read(cx);
        let count = if deck.is_hidden() { 0 } else { deck.len() };
        let fanned = deck.fanned();
        if count == 0 {
            if let Some(shown) = self.shown.borrow_mut().take() {
                shown.handle.close(cx);
            }
            return Ok(());
        }
        let settings = self.settings.get();
        let geometry = settings.geometry(count);
        let mut slot = self.shown.borrow_mut();
        match slot.as_mut() {
            Some(shown) if shown.count == count && shown.fanned == fanned => return Ok(()),
            Some(shown) => {
                shown.fanned = fanned;
                if shown.count != count {
                    shown.count = count;
                    shown.handle.resize(geometry.panel(), cx);
                }
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
                let deck = self.deck.clone();
                let settled = self.rebuilt.replace(false);
                let view = cx.new(|cx| {
                    let view = DeckView::new(deck, settings, cx);
                    if settled { view.settled() } else { view }
                });
                handle.set_content(view.into(), cx);
                *slot = Some(Shown {
                    handle,
                    count,
                    fanned,
                });
            }
        }
        if let Some(shown) = slot.as_ref() {
            let output = shown.handle.output_info().logical;
            shown.handle.set_position(geometry.origin(output), cx);
            shown
                .handle
                .set_input_region(&geometry.input_region(fanned), cx);
        }
        Ok(())
    }
}
