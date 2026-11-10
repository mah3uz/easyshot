use super::surfaces::{LayerShellSurfaces, LayerSurfaceHandle};
use super::{SurfaceHandle as _, SurfaceRole};
use crate::errors::{Error, Result};
use crate::toast::{Countdown, ToastRequest, ToastSink, ToastStyle};
use es_geom::Size;
use es_model::platform::OutputId;
use es_ui::components::{Toast, ToastStyle as CardStyle};
use es_ui::icons::IconName;
use es_ui::motion::TOAST_OUT;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, Render, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};
use gpui_component::IconNamed as _;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const SURFACE: Size = Size::new(420., 132.);
const TICK: Duration = Duration::from_millis(50);
const TOP_GAP: Pixels = px(12.);

struct ToastHost {
    id: u64,
    toast: ToastRequest,
    leaving: bool,
    region: Rc<Cell<Option<Bounds<Pixels>>>>,
    hovered: Rc<Cell<bool>>,
    shown: Rc<RefCell<Option<Shown>>>,
}

impl Render for ToastHost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (id, shown) = (self.id, self.shown.clone());
        let t = &self.toast;
        let style = match t.style {
            ToastStyle::Normal => CardStyle::Normal,
            ToastStyle::Error => CardStyle::Error,
        };
        let mut card = Toast::new(("toast", self.id), t.title.clone())
            .style(style)
            .leaving(self.leaving);
        if let Some(message) = &t.message {
            card = card.message(message.clone());
        }
        if let Some(icon) = card_icon(t.icon.clone()) {
            card = card.icon(icon);
        }
        if let Some((label, undo)) = t.undo.clone() {
            card = card.action(label, move |_, cx| {
                undo.run(cx);
                let shown = shown.clone();
                cx.defer(move |cx| close(&shown, id, cx));
            });
        }
        let region = self.region.clone();
        let hovered = self.hovered.clone();
        div().size_full().flex().justify_center().pt(TOP_GAP).child(
            div()
                .on_children_prepainted(move |bounds, window, _| {
                    let card = bounds.first().copied();
                    if card.is_some() && card != region.get() {
                        region.set(card);
                        window.set_input_region(card.as_ref().map(std::slice::from_ref));
                    }
                })
                .id(("toast-card", self.id))
                .on_hover(move |over, _, _| hovered.set(*over))
                .child(card),
        )
    }
}

// The card draws the catalogued icons only; an uncatalogued one leaves the card without an icon.
pub fn card_icon(icon: gpui_component::IconName) -> Option<IconName> {
    let path = icon.path();
    let stem = path.rsplit('/').next()?.strip_suffix(".svg")?;
    IconName::ALL.iter().copied().find(|i| i.name() == stem)
}

struct Shown {
    id: u64,
    handle: LayerSurfaceHandle,
    host: Entity<ToastHost>,
}

/// One toast at a time; a new one replaces the one on screen.
pub struct ToastSurface {
    surfaces: Rc<LayerShellSurfaces>,
    shown: Rc<RefCell<Option<Shown>>>,
    next: Cell<u64>,
}

impl ToastSurface {
    pub fn new(surfaces: Rc<LayerShellSurfaces>) -> Self {
        ToastSurface {
            surfaces,
            shown: Rc::default(),
            next: Cell::new(1),
        }
    }

    pub fn try_show(&self, toast: &ToastRequest, cx: &mut App) -> Result<OutputId> {
        let output = match toast.output.clone() {
            Some(output) => output,
            None => self
                .surfaces
                .test_output()
                .cloned()
                .or_else(|| self.surfaces.source().focused_output())
                .ok_or_else(|| Error::NotFound("an output for the toast".into()))?,
        };
        self.dismiss(cx);
        let id = self.next.get();
        self.next.set(id + 1);
        let duration = toast.duration;
        let handle = self
            .surfaces
            .open_surface(SurfaceRole::Toast, &output, SURFACE, cx)?;
        let toast = toast.clone();
        let hovered = Rc::new(Cell::new(false));
        let host = cx.new(|_| ToastHost {
            id,
            toast,
            leaving: false,
            region: Rc::default(),
            hovered: hovered.clone(),
            shown: self.shown.clone(),
        });
        handle.set_content(host.clone().into(), cx);
        *self.shown.borrow_mut() = Some(Shown { id, handle, host });
        let shown = self.shown.clone();
        cx.spawn(async move |cx| {
            let mut countdown = Countdown::new(duration);
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(TICK).await;
                let now = Instant::now();
                if countdown.advance(now - last, hovered.get()) {
                    break;
                }
                last = now;
            }
            let still = cx.update(|cx| leave(&shown, id, cx));
            if still {
                cx.background_executor().timer(TOAST_OUT.duration).await;
                cx.update(|cx| close(&shown, id, cx));
            }
        })
        .detach();
        Ok(output)
    }

    pub fn dismiss(&self, cx: &mut App) {
        if let Some(old) = self.shown.borrow_mut().take() {
            old.handle.close(cx);
        }
    }
}

fn leave(shown: &RefCell<Option<Shown>>, id: u64, cx: &mut App) -> bool {
    let Some(host) = shown
        .borrow()
        .as_ref()
        .filter(|s| s.id == id)
        .map(|s| s.host.clone())
    else {
        return false;
    };
    host.update(cx, |host, cx| {
        host.leaving = true;
        cx.notify();
    });
    true
}

fn close(shown: &RefCell<Option<Shown>>, id: u64, cx: &mut App) {
    let mut slot = shown.borrow_mut();
    if slot.as_ref().is_some_and(|s| s.id == id)
        && let Some(s) = slot.take()
    {
        drop(slot);
        s.handle.close(cx);
    }
}

impl ToastSink for ToastSurface {
    fn show(&self, request: &ToastRequest, cx: &mut App) -> bool {
        match self.try_show(request, cx) {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!(%error, "toast not shown");
                false
            }
        }
    }
}
