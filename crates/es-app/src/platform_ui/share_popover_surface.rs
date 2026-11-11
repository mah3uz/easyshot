use super::surfaces::{LayerShellSurfaces, LayerSurfaceHandle};
use super::{SurfaceHandle as _, SurfaceRole};
use crate::errors::{Error, Result};
use crate::fl;
use crate::share::flow::BuildPopover;
use crate::share::{Anchor, PopoverHost};
use es_geom::{Point, Rect, Size};
use es_ui::components::{Popover, SHARE_POPOVER_WIDTH, SharePopover};
use gpui::{
    App, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Render,
    Styled as _, Window, div, px,
};
use std::cell::RefCell;
use std::rc::Rc;

const FALLBACK_TOP: f64 = 56.;

type Shown = Option<(LayerSurfaceHandle, Rc<dyn Fn(&mut App)>)>;

/// The popover over a clear surface that covers its output, so a press anywhere else closes it.
pub struct SharePopoverPanel {
    trigger: Rect,
    opens_left: bool,
    build: Option<BuildPopover>,
    popover: Option<Entity<SharePopover>>,
    close: Rc<dyn Fn(&mut App)>,
}

impl SharePopoverPanel {
    /// `trigger` is in the panel's own pixels; the popover opens above it, toward the wider side.
    pub fn new(
        trigger: Rect,
        width: f64,
        build: BuildPopover,
        close: Rc<dyn Fn(&mut App)>,
    ) -> Self {
        Self {
            trigger,
            opens_left: trigger.center().x > width / 2.,
            build: Some(build),
            popover: None,
            close,
        }
    }
}

impl Render for SharePopoverPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(build) = self.build.take() {
            self.popover = Some(build(window, cx));
        }
        let Some(popover) = self.popover.clone() else {
            return div().size_full();
        };
        let focus = popover.read(cx).focus_handle(cx);
        let close = self.close.clone();
        let anchor = if self.opens_left {
            gpui::Anchor::BottomRight
        } else {
            gpui::Anchor::BottomLeft
        };
        let r = self.trigger;
        let (w, h) = (px(r.width() as f32), px(r.height() as f32));
        div().size_full().child(
            div()
                .absolute()
                .left(px(r.x0 as f32))
                .top(px(r.y0 as f32))
                .w(w)
                .h(h)
                .child(
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
                        .trigger(div().w(w).h(h))
                        .content(move |_, _| popover.clone()),
                ),
        )
    }
}

/// One share popover at a time, on the output of the control it points at.
pub struct SharePopoverSurface {
    surfaces: Rc<LayerShellSurfaces>,
    shown: Rc<RefCell<Shown>>,
}

impl SharePopoverSurface {
    pub fn new(surfaces: Rc<LayerShellSurfaces>) -> Self {
        Self {
            surfaces,
            shown: Rc::default(),
        }
    }
}

fn take_and_close(shown: &RefCell<Shown>, cx: &mut App) {
    let taken = shown.borrow_mut().take();
    if let Some((handle, closed)) = taken {
        handle.close(cx);
        closed(cx);
    }
}

impl PopoverHost for SharePopoverSurface {
    fn open(
        &self,
        anchor: Option<&Anchor>,
        build: BuildPopover,
        closed: Rc<dyn Fn(&mut App)>,
        cx: &mut App,
    ) -> Result<()> {
        self.close(cx);
        let source = self.surfaces.source();
        let output = anchor
            .map(|a| a.output.clone())
            .or_else(|| self.surfaces.test_output().cloned())
            .or_else(|| source.pointer_output())
            .or_else(|| source.focused_output())
            .ok_or_else(|| Error::NotFound("an output for the share popover".into()))?;
        let area = self.surfaces.output_info(&output)?.logical;
        let size = Size::new(area.width(), area.height());
        let trigger = anchor.map(|a| a.rect).unwrap_or_else(|| {
            Rect::from_origin_size(Point::new(size.width / 2., FALLBACK_TOP), Size::new(1., 1.))
        });
        let handle = self
            .surfaces
            .open_surface(SurfaceRole::Panel, &output, size, cx)?;
        let shown = self.shown.clone();
        let close: Rc<dyn Fn(&mut App)> = Rc::new(move |cx| take_and_close(&shown, cx));
        let panel = cx.new(|_| SharePopoverPanel::new(trigger, size.width, build, close));
        handle.set_content(panel.into(), cx);
        *self.shown.borrow_mut() = Some((handle, closed));
        Ok(())
    }

    fn close(&self, cx: &mut App) {
        take_and_close(&self.shown, cx);
    }
}
