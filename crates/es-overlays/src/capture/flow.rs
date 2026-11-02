use super::geometry;
use super::selector::{RegionSelector, SelectorEvent};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SelectMode, Selection, SelectionFlow, SurfaceHandle,
    SurfaceRole,
};
use es_backend::Backend;
use es_geom::Rect;
use es_model::LastRegion;
use es_model::platform::{CaptureOpts, Frame, OutputId};
use gpui::{App, AppContext as _, AsyncApp, Entity, Subscription, Task};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowOptions {
    pub capture_on_release: bool,
    pub preselect_previous: bool,
}

impl Default for FlowOptions {
    fn default() -> Self {
        FlowOptions {
            capture_on_release: false,
            preselect_previous: true,
        }
    }
}

pub type Options = Rc<dyn Fn(&App) -> FlowOptions>;
pub type Recall = Rc<dyn Fn(&App) -> Option<LastRegion>>;
pub type Remember = Rc<dyn Fn(LastRegion, &mut App)>;

pub struct LayerShellSelectionFlow {
    surfaces: Rc<LayerShellSurfaces>,
    backend: Arc<Backend>,
    options: Options,
    recall: Recall,
    remember: Remember,
}

impl LayerShellSelectionFlow {
    pub fn new(surfaces: Rc<LayerShellSurfaces>, backend: Arc<Backend>) -> Self {
        LayerShellSelectionFlow {
            surfaces,
            backend,
            options: Rc::new(|_| FlowOptions::default()),
            recall: Rc::new(|_| None),
            remember: Rc::new(|_, _| {}),
        }
    }

    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    /// The previous area comes from state; `remember` hands the new one back to be written there.
    pub fn with_last_region(mut self, recall: Recall, remember: Remember) -> Self {
        self.recall = recall;
        self.remember = remember;
        self
    }
}

struct Overlay {
    output: OutputId,
    selector: Entity<RegionSelector>,
    handle: LayerSurfaceHandle,
}

/// Closes every overlay however the selection ends, including when the task is dropped.
struct Open {
    overlays: Vec<Overlay>,
    _subscriptions: Vec<Subscription>,
    cx: AsyncApp,
}

impl Drop for Open {
    fn drop(&mut self) {
        let overlays = std::mem::take(&mut self.overlays);
        self.cx.update(|cx| {
            for overlay in &overlays {
                overlay.handle.close(cx);
            }
        });
    }
}

fn preselect(last: Option<&LastRegion>, output: &OutputId) -> Option<Rect> {
    let r = last.filter(|r| r.output == output.0)?;
    let (x, y) = (f64::from(r.x), f64::from(r.y));
    Some(Rect::new(x, y, x + f64::from(r.w), y + f64::from(r.h)))
}

fn remembered(output: &OutputId, local: Rect) -> LastRegion {
    let r = local.round();
    LastRegion {
        output: output.0.clone(),
        x: r.x0 as i32,
        y: r.y0 as i32,
        w: r.width().max(0.0) as u32,
        h: r.height().max(0.0) as u32,
    }
}

impl LayerShellSelectionFlow {
    fn open(
        &self,
        frames: Vec<Frame>,
        cx: &mut App,
    ) -> Result<(Open, async_channel::Receiver<(usize, SelectorEvent)>)> {
        let options = (self.options)(cx);
        let last = options
            .preselect_previous
            .then(|| (self.recall)(cx))
            .flatten();
        let mut open = Open {
            overlays: Vec::new(),
            _subscriptions: Vec::new(),
            cx: cx.to_async(),
        };
        let pointer = self.surfaces.source().pointer_output();
        let mut frames: Vec<(OutputId, Frame)> = frames
            .into_iter()
            .filter_map(|f| Some((f.output.clone()?, f)))
            .collect();
        frames.sort_by_key(|(id, _)| Some(id) != pointer.as_ref());
        for (output, frame) in frames {
            let info = self.surfaces.output_info(&output)?;
            let handle = self.surfaces.open_surface(
                SurfaceRole::Overlay,
                &output,
                info.logical.size(),
                cx,
            )?;
            let pre = preselect(last.as_ref(), &output);
            let frame = Arc::new(frame);
            let selector =
                cx.new(|cx| RegionSelector::new(frame, pre, options.capture_on_release, cx));
            handle.set_content(selector.clone().into(), cx);
            open.overlays.push(Overlay {
                output,
                selector,
                handle,
            });
        }
        if open.overlays.is_empty() {
            return Err(Error::NotFound("any output to select on".into()));
        }
        let (tx, rx) = async_channel::unbounded();
        let selectors: Vec<Entity<RegionSelector>> =
            open.overlays.iter().map(|o| o.selector.clone()).collect();
        for (index, selector) in selectors.iter().enumerate() {
            let (tx, others) = (tx.clone(), selectors.clone());
            open._subscriptions
                .push(cx.subscribe(selector, move |_, event, cx| {
                    if *event == SelectorEvent::Began {
                        for (i, other) in others.iter().enumerate() {
                            if i != index {
                                other.update(cx, |s, cx| s.clear(cx));
                            }
                        }
                    }
                    let _ = tx.try_send((index, *event));
                }));
        }
        Ok((open, rx))
    }
}

impl SelectionFlow for LayerShellSelectionFlow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        if mode != SelectMode::Region {
            return Task::ready(Err(Error::Unsupported));
        }
        let ids: Vec<OutputId> = self
            .surfaces
            .source()
            .outputs()
            .into_iter()
            .map(|o| o.id)
            .collect();
        let backend = self.backend.clone();
        let frozen = cx.background_spawn(async move {
            backend
                .capture
                .capture_outputs(&ids, CaptureOpts { cursor: false })
        });
        let this = LayerShellSelectionFlow {
            surfaces: self.surfaces.clone(),
            backend: self.backend.clone(),
            options: self.options.clone(),
            recall: self.recall.clone(),
            remember: self.remember.clone(),
        };
        cx.spawn(async move |cx| {
            let frames = frozen.await.map_err(|e| Error::Protocol(e.to_string()))?;
            let (open, rx) = cx.update(|cx| this.open(frames, cx))?;
            loop {
                let (index, event) = rx.recv().await.map_err(|_| Error::Cancelled)?;
                let Some(overlay) = open.overlays.get(index) else {
                    continue;
                };
                match event {
                    SelectorEvent::Began | SelectorEvent::WindowMode => {}
                    SelectorEvent::Cancel => return Ok(Selection::Cancelled),
                    SelectorEvent::Capture(local) => {
                        let output = overlay.output.clone();
                        let frame = cx.update(|cx| overlay.selector.read(cx).frame().clone());
                        let cropped = geometry::crop(&frame, local)
                            .ok_or_else(|| Error::Internal("empty selection".into()))?;
                        let remember = this.remember.clone();
                        let last = remembered(&output, local);
                        drop(open);
                        cx.update(|cx| remember(last, cx));
                        return Ok(Selection::Region {
                            output,
                            logical: cropped.logical,
                            frame: Arc::new(cropped),
                        });
                    }
                }
            }
        })
    }
}
