use super::surfaces::{LayerShellSurfaces, LayerSurfaceHandle};
use super::{SurfaceHandle as _, SurfaceRole};
use crate::errors::{Error, Result};
use es_geom::Size;
use es_ui::components::{ComingSoon, Feature};
use es_ui::motion::PANEL_OUT;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, Styled as _, Window, div,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The panel on its surface: Esc, the close button and a press outside the panel all close it.
pub struct ComingSoonHost {
    feature: Feature,
    focus: FocusHandle,
    focused: bool,
    leaving: bool,
    close: Rc<dyn Fn(&mut App)>,
}

impl Focusable for ComingSoonHost {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ComingSoonHost {
    /// `close` runs once the exit has played.
    pub fn new(feature: Feature, close: impl Fn(&mut App) + 'static, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| ComingSoonHost {
            feature,
            focus: cx.focus_handle(),
            focused: false,
            leaving: false,
            close: Rc::new(close),
        })
    }

    pub fn feature(&self) -> Feature {
        self.feature
    }

    pub fn leaving(&self) -> bool {
        self.leaving
    }

    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.leaving {
            return;
        }
        self.leaving = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PANEL_OUT.duration).await;
            let _ = this.update(cx, |host, cx| (host.close.clone())(cx));
        })
        .detach();
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        if e.keystroke.key == "escape" {
            self.dismiss(cx);
        }
    }
}

impl Render for ComingSoonHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let id = cx.entity_id().as_u64();
        let this = cx.entity().downgrade();
        div()
            .id("coming-soon-host")
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|host, e, _, cx| host.key(e, cx)))
            .child(
                ComingSoon::new(("coming-soon", id), self.feature)
                    .leaving(self.leaving)
                    .on_close(move |_, cx| {
                        let _ = this.update(cx, |host, cx| host.dismiss(cx));
                    }),
            )
    }
}

struct Shown {
    id: u64,
    handle: LayerSurfaceHandle,
}

/// One panel at a time, on the output under the pointer; a new one replaces the one on screen.
pub struct ComingSoonSurface {
    surfaces: Rc<LayerShellSurfaces>,
    shown: Rc<RefCell<Option<Shown>>>,
    next: Cell<u64>,
}

impl ComingSoonSurface {
    pub fn new(surfaces: Rc<LayerShellSurfaces>) -> Self {
        ComingSoonSurface {
            surfaces,
            shown: Rc::default(),
            next: Cell::new(1),
        }
    }

    pub fn show(&self, feature: Feature, cx: &mut App) -> Result<()> {
        let source = self.surfaces.source();
        let output = self
            .surfaces
            .test_output()
            .cloned()
            .or_else(|| source.pointer_output())
            .or_else(|| source.focused_output())
            .ok_or_else(|| Error::NotFound("an output for the panel".into()))?;
        let area = self.surfaces.output_info(&output)?.logical;
        if let Some(old) = self.shown.borrow_mut().take() {
            old.handle.close(cx);
        }
        let id = self.next.get();
        self.next.set(id + 1);
        let handle = self.surfaces.open_surface(
            SurfaceRole::Panel,
            &output,
            Size::new(area.width(), area.height()),
            cx,
        )?;
        let shown = self.shown.clone();
        let host = ComingSoonHost::new(feature, move |cx| close(&shown, id, cx), cx);
        handle.set_content(host.into(), cx);
        *self.shown.borrow_mut() = Some(Shown { id, handle });
        Ok(())
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
