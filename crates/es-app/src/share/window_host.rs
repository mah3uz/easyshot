use super::flow::{Anchor, BuildPopover, PopoverHost};
use crate::errors::Result;
use crate::fl;
use es_ui::components::{Popover, SHARE_POPOVER_WIDTH, SharePopover};
use gpui::{AnyElement, App, Context, Entity, EntityId, Focusable as _, IntoElement};
use std::cell::RefCell;
use std::rc::Rc;

type Closed = Rc<dyn Fn(&mut App)>;

#[derive(Default)]
struct State {
    build: Option<BuildPopover>,
    popover: Option<Entity<SharePopover>>,
    closed: Option<Closed>,
    owner: Option<EntityId>,
}

/// The share popover inside a surface's own window, opening from its Share button.
#[derive(Clone, Default)]
pub struct WindowPopover(Rc<RefCell<State>>);

impl WindowPopover {
    pub fn is_open(&self) -> bool {
        let s = self.0.borrow();
        s.build.is_some() || s.popover.is_some()
    }

    /// Wraps the Share button; `anchor` is the popover's corner that meets the button.
    pub fn around<V: 'static>(
        &self,
        button: impl IntoElement,
        anchor: gpui::Anchor,
        window: &mut gpui::Window,
        cx: &mut Context<V>,
    ) -> AnyElement {
        self.0.borrow_mut().owner = Some(cx.entity_id());
        let build = self.0.borrow_mut().build.take();
        if let Some(build) = build {
            let popover = build(window, cx);
            self.0.borrow_mut().popover = Some(popover);
        }
        let Some(popover) = self.0.borrow().popover.clone() else {
            return button.into_any_element();
        };
        let this = self.clone();
        popover_from(
            button,
            popover,
            anchor,
            Rc::new(move |cx| this.close(cx)),
            cx,
        )
        .into_any_element()
    }

    fn redraw(&self, cx: &mut App) {
        let owner = self.0.borrow().owner;
        if let Some(owner) = owner {
            cx.notify(owner);
        }
    }
}

impl PopoverHost for WindowPopover {
    fn open(
        &self,
        _: Option<&Anchor>,
        build: BuildPopover,
        closed: Rc<dyn Fn(&mut App)>,
        cx: &mut App,
    ) -> Result<()> {
        self.close(cx);
        {
            let mut s = self.0.borrow_mut();
            s.build = Some(build);
            s.closed = Some(closed);
        }
        self.redraw(cx);
        Ok(())
    }

    fn close(&self, cx: &mut App) {
        let closed = {
            let mut s = self.0.borrow_mut();
            s.build = None;
            s.popover = None;
            s.closed.take()
        };
        if let Some(closed) = closed {
            closed(cx);
            self.redraw(cx);
        }
    }
}

/// The share popover opening from `trigger`; `close` runs however it is dismissed.
pub fn popover_from(
    trigger: impl IntoElement,
    popover: Entity<SharePopover>,
    anchor: gpui::Anchor,
    close: Rc<dyn Fn(&mut App)>,
    cx: &App,
) -> Popover {
    let focus = popover.read(cx).focus_handle(cx);
    Popover::new("share-popover", fl!("share-popover-a11y"))
        .width(SHARE_POPOVER_WIDTH)
        .anchor(anchor)
        .open(true)
        .focus(&focus)
        .on_open_change(move |open, _, cx| {
            if !open {
                close(cx);
            }
        })
        .trigger(trigger)
        .content(move |_, _| popover.clone())
}
