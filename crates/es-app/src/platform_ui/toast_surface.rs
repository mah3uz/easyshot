use super::surfaces::{LayerShellSurfaces, LayerSurfaceHandle};
use super::{SurfaceHandle as _, SurfaceRole};
use crate::errors::{Error, Result};
use crate::toast::{CloseReason, ToastRequest, ToastSink, ToastStyle};
use es_geom::Size;
use es_model::platform::OutputId;
use es_ui::components::{Toast, ToastClock, ToastStyle as CardStyle};
use es_ui::icons::IconName;
use es_ui::motion::TOAST_OUT;
use gpui::{
    App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Pixels, Render,
    Styled as _, Window, div, px,
};
use gpui_component::IconNamed as _;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const SURFACE: Size = Size::new(420., 132.);
const TOP_GAP: Pixels = px(12.);

/// The card on its surface, and its countdown.
pub struct ToastHost {
    toast: ToastRequest,
    leaving: bool,
    region: Rc<Cell<Option<Bounds<Pixels>>>>,
    clock: Option<ToastClock>,
    close: Rc<dyn Fn(&mut App)>,
}

impl ToastHost {
    /// Runs down while the pointer is off the card, then plays the exit and calls `close`.
    pub fn start(
        toast: ToastRequest,
        close: impl Fn(&mut App) + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let clock = Self::clock(&toast, cx);
            ToastHost {
                toast,
                leaving: false,
                region: Rc::default(),
                clock,
                close: Rc::new(close),
            }
        })
    }

    fn clock(toast: &ToastRequest, cx: &mut Context<Self>) -> Option<ToastClock> {
        if toast.sticky {
            return None;
        }
        let label = format!("{} {}", cx.entity_id().as_u64(), toast.title);
        Some(ToastClock::start(
            label,
            toast.duration,
            cx,
            |host: &mut ToastHost, cx| {
                host.toast.close(CloseReason::Expired, cx);
                host.leave(cx);
            },
        ))
    }

    fn leave(&mut self, cx: &mut Context<Self>) {
        self.leaving = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_OUT.duration).await;
            let _ = this.update(cx, |host, cx| (host.close.clone())(cx));
        })
        .detach();
    }

    /// Keeps the card where it is; the countdown starts again only when the request has one.
    pub fn update(&mut self, toast: &ToastRequest, cx: &mut Context<Self>) {
        let restart = !toast.sticky && (self.toast.sticky || self.toast.duration != toast.duration);
        self.toast = ToastRequest {
            closed: self.toast.closed.clone(),
            ..toast.clone()
        };
        if toast.sticky {
            self.clock = None;
        } else if restart {
            self.clock = Self::clock(toast, cx);
        }
        cx.notify();
    }

    pub fn leaving(&self) -> bool {
        self.leaving
    }
}

impl Render for ToastHost {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = cx.entity_id().as_u64();
        let t = &self.toast;
        let style = match t.style {
            ToastStyle::Normal => CardStyle::Normal,
            ToastStyle::Error => CardStyle::Error,
        };
        let mut card = Toast::new(("toast", id), t.title.clone())
            .style(style)
            .leaving(self.leaving);
        if let Some(clock) = &self.clock {
            card = card.clock(clock);
        }
        if let Some(message) = &t.message {
            card = card.message(message.clone());
        }
        if let Some(icon) = card_icon(t.icon.clone()) {
            card = card.icon(icon);
        }
        if let Some(done) = t.progress {
            card = card.progress(done);
        }
        let undo = t
            .undo
            .clone()
            .map(|(label, act)| (label, act, CloseReason::Undone));
        let others = t
            .actions
            .iter()
            .map(|(label, act)| (label.clone(), act.clone(), CloseReason::Acted));
        for (label, act, reason) in undo.into_iter().chain(others) {
            let close = self.close.clone();
            let request = t.clone();
            card = card.action(label, move |_, cx| {
                act.run(cx);
                request.close(reason, cx);
                let close = close.clone();
                cx.defer(move |cx| close(cx));
            });
        }
        let region = self.region.clone();
        div().size_full().flex().justify_center().pt(TOP_GAP).child(
            div()
                .on_children_prepainted(move |bounds, window, _| {
                    let card = bounds.first().copied();
                    if card.is_some() && card != region.get() {
                        region.set(card);
                        window.set_input_region(card.as_ref().map(std::slice::from_ref));
                    }
                })
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
    request: ToastRequest,
    host: Entity<ToastHost>,
    handle: LayerSurfaceHandle,
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
        if let Some(output) = self.update_shown(toast, cx) {
            return Ok(output);
        }
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
        let handle = self
            .surfaces
            .open_surface(SurfaceRole::Toast, &output, SURFACE, cx)?;
        let shown = self.shown.clone();
        let host = ToastHost::start(toast.clone(), move |cx| close(&shown, id, cx), cx);
        handle.set_content(host.clone().into(), cx);
        *self.shown.borrow_mut() = Some(Shown {
            id,
            request: toast.clone(),
            host,
            handle,
        });
        Ok(output)
    }

    fn update_shown(&self, toast: &ToastRequest, cx: &mut App) -> Option<OutputId> {
        let (host, output) = {
            let mut slot = self.shown.borrow_mut();
            let shown = slot
                .as_mut()
                .filter(|s| toast.key.is_some() && s.request.key == toast.key)?;
            if shown.host.read(cx).leaving() {
                return None;
            }
            shown.request = ToastRequest {
                closed: shown.request.closed.clone(),
                ..toast.clone()
            };
            (shown.host.clone(), shown.handle.output())
        };
        host.update(cx, |host, cx| host.update(toast, cx));
        Some(output)
    }

    pub fn dismiss(&self, cx: &mut App) {
        let old = self.shown.borrow_mut().take();
        if let Some(old) = old {
            old.request.close(CloseReason::Replaced, cx);
            old.handle.close(cx);
        }
    }
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

    fn update(&self, request: &ToastRequest, cx: &mut App) -> bool {
        self.update_shown(request, cx).is_some()
    }

    fn dismiss(&self, key: u64, cx: &mut App) {
        let ours = self
            .shown
            .borrow()
            .as_ref()
            .is_some_and(|s| s.request.key == Some(key));
        if ours {
            ToastSurface::dismiss(self, cx);
        }
    }
}
