use super::color::{ColorEvent, ColorPicker};
use super::geometry;
use super::picker::WindowPicker;
use super::scroll_flow::{self, Control};
use super::selector::{RegionSelector, SelectorEvent, SelectorStyle};
use super::window::{PickEvent, WindowSelector, window_frame};
use super::windows;
use es_app::capture_coordinator::Hook;
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SelectMode, Selection, SelectionFlow, SurfaceHandle,
    SurfaceRole,
};
use es_backend::Backend;
use es_geom::{Rect, Rgba};
use es_model::LastRegion;
use es_model::platform::{CaptureOpts, Frame, OutputId, WindowId, WindowInfo};
use gpui::{AnyView, App, AppContext as _, AsyncApp, Entity, Subscription, Task};
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
    scroll: Control,
}

impl LayerShellSelectionFlow {
    pub fn new(surfaces: Rc<LayerShellSurfaces>, backend: Arc<Backend>) -> Self {
        LayerShellSelectionFlow {
            surfaces,
            backend,
            options: Rc::new(|_| FlowOptions::default()),
            recall: Rc::new(|_| None),
            remember: Rc::new(|_, _| {}),
            scroll: Control::default(),
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

    /// Finishes the running scroll capture as Stop does.
    pub fn scroll_stop(&self) -> Hook {
        let control = self.scroll.clone();
        Rc::new(move |_| {
            if let Some(tx) = control.borrow().as_ref() {
                let _ = tx.send(super::scroll::Command::Stop);
            }
        })
    }

    fn duplicate(&self) -> Self {
        LayerShellSelectionFlow {
            surfaces: self.surfaces.clone(),
            backend: self.backend.clone(),
            options: self.options.clone(),
            recall: self.recall.clone(),
            remember: self.remember.clone(),
            scroll: self.scroll.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Region,
    Text,
    Window,
    Color,
    Scroll,
}

#[derive(Debug, Clone, PartialEq)]
enum Pick {
    Began,
    Region(Rect),
    Window { id: WindowId, local: Option<Rect> },
    Color(Rgba),
    Cancel,
    ToWindow,
    ToRegion,
}

enum Content {
    Region(Entity<RegionSelector>),
    Window(Entity<WindowSelector>),
    Picker(Entity<WindowPicker>),
    Color(Entity<ColorPicker>),
}

impl Content {
    fn view(&self) -> AnyView {
        match self {
            Content::Region(e) => e.clone().into(),
            Content::Window(e) => e.clone().into(),
            Content::Picker(e) => e.clone().into(),
            Content::Color(e) => e.clone().into(),
        }
    }
}

struct Overlay {
    output: OutputId,
    /// Global logical rectangle of the output.
    logical: Rect,
    frame: Arc<Frame>,
    handle: LayerSurfaceHandle,
    content: Option<Content>,
}

struct Setup {
    options: FlowOptions,
    last: Option<LastRegion>,
    windows: Vec<WindowInfo>,
    geometry: bool,
}

/// Closes every overlay however the selection ends, including when the task is dropped.
struct Open {
    overlays: Vec<Overlay>,
    subscriptions: Vec<Subscription>,
    tx: async_channel::Sender<(usize, Pick)>,
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

fn pick(event: &PickEvent) -> Pick {
    match event {
        PickEvent::Window { id, local } => Pick::Window {
            id: id.clone(),
            local: *local,
        },
        PickEvent::Cancel => Pick::Cancel,
        PickEvent::Region => Pick::ToRegion,
    }
}

impl Open {
    fn content(&self, view: View, index: usize, setup: &Setup, cx: &mut App) -> Option<Content> {
        let o = self.overlays.get(index)?;
        let frame = o.frame.clone();
        let pid = std::process::id();
        Some(match view {
            View::Region | View::Text | View::Scroll => {
                let text = view == View::Text;
                let pre = (view == View::Region)
                    .then(|| preselect(setup.last.as_ref(), &o.output))
                    .flatten();
                let release = text || setup.options.capture_on_release;
                let style = if text {
                    SelectorStyle::Text
                } else {
                    SelectorStyle::Region
                };
                Content::Region(
                    cx.new(|cx| RegionSelector::new(frame, pre, release, cx).with_style(style)),
                )
            }
            // The list goes on the pointer's output only; the others just show the frozen screen.
            View::Window if !setup.geometry => {
                let items = (index == 0).then(|| windows::listed(&setup.windows, pid));
                Content::Picker(cx.new(|cx| WindowPicker::new(&frame, items, cx)))
            }
            View::Window => {
                let here = windows::on_output(&setup.windows, &o.output, o.logical, pid);
                Content::Window(cx.new(|cx| WindowSelector::new(frame, here, cx)))
            }
            View::Color => Content::Color(cx.new(|cx| ColorPicker::new(frame, cx))),
        })
    }

    fn show(&mut self, view: View, setup: &Setup, cx: &mut App) {
        self.subscriptions.clear();
        for index in 0..self.overlays.len() {
            let content = self.content(view, index, setup, cx);
            if let (Some(o), Some(content)) = (self.overlays.get_mut(index), content) {
                o.handle.set_content(content.view(), cx);
                o.content = Some(content);
            }
        }
        let regions: Vec<Entity<RegionSelector>> = self
            .overlays
            .iter()
            .filter_map(|o| match &o.content {
                Some(Content::Region(e)) => Some(e.clone()),
                _ => None,
            })
            .collect();
        for (index, o) in self.overlays.iter().enumerate() {
            let tx = self.tx.clone();
            let send = move |pick: Pick| {
                let _ = tx.try_send((index, pick));
            };
            let subscription = match &o.content {
                Some(Content::Region(e)) => {
                    let (me, others) = (e.clone(), regions.clone());
                    cx.subscribe(e, move |_, event: &SelectorEvent, cx| {
                        if *event == SelectorEvent::Began {
                            for other in others.iter().filter(|o| **o != me) {
                                other.update(cx, |s, cx| s.clear(cx));
                            }
                        }
                        send(match *event {
                            SelectorEvent::Began => Pick::Began,
                            SelectorEvent::Capture(r) => Pick::Region(r),
                            SelectorEvent::Cancel => Pick::Cancel,
                            SelectorEvent::WindowMode => Pick::ToWindow,
                        });
                    })
                }
                Some(Content::Window(e)) => {
                    cx.subscribe(e, move |_, event: &PickEvent, _| send(pick(event)))
                }
                Some(Content::Picker(e)) => {
                    cx.subscribe(e, move |_, event: &PickEvent, _| send(pick(event)))
                }
                Some(Content::Color(e)) => cx.subscribe(e, move |_, event: &ColorEvent, _| {
                    send(match *event {
                        ColorEvent::Picked(c) => Pick::Color(c),
                        ColorEvent::Cancel => Pick::Cancel,
                    })
                }),
                None => continue,
            };
            self.subscriptions.push(subscription);
        }
    }
}

impl LayerShellSelectionFlow {
    fn open(
        &self,
        frames: Vec<Frame>,
        cx: &mut App,
    ) -> Result<(Open, async_channel::Receiver<(usize, Pick)>)> {
        let (tx, rx) = async_channel::unbounded();
        let mut open = Open {
            overlays: Vec::new(),
            subscriptions: Vec::new(),
            tx,
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
            open.overlays.push(Overlay {
                output,
                logical: info.logical,
                frame: Arc::new(frame),
                handle,
                content: None,
            });
        }
        if open.overlays.is_empty() {
            return Err(Error::NotFound("any output to select on".into()));
        }
        Ok((open, rx))
    }
}

fn view_for(mode: SelectMode) -> Option<View> {
    match mode {
        SelectMode::Region => Some(View::Region),
        SelectMode::Text => Some(View::Text),
        SelectMode::Window => Some(View::Window),
        SelectMode::Color => Some(View::Color),
        SelectMode::Scroll => Some(View::Scroll),
    }
}

impl SelectionFlow for LayerShellSelectionFlow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        let Some(first) = view_for(mode) else {
            return Task::ready(Err(Error::Unsupported));
        };
        let ids: Vec<OutputId> = self
            .surfaces
            .source()
            .outputs()
            .into_iter()
            .map(|o| o.id)
            .collect();
        let backend = self.backend.clone();
        let wants_windows = matches!(first, View::Region | View::Window);
        let frozen = cx.background_spawn(async move {
            let windows = if wants_windows {
                (backend.windows.snapshot(), backend.windows.has_geometry())
            } else {
                (Vec::new(), false)
            };
            let frames = backend
                .capture
                .capture_outputs(&ids, CaptureOpts { cursor: false });
            (frames, windows)
        });
        let options = (self.options)(cx);
        let last = options
            .preselect_previous
            .then(|| (self.recall)(cx))
            .flatten();
        let this = self.duplicate();
        cx.spawn(async move |cx| {
            let (frames, (windows, geometry)) = frozen.await;
            let frames = frames.map_err(|e| Error::Protocol(e.to_string()))?;
            if first == View::Window && windows.is_empty() && !geometry {
                return Err(Error::Unsupported);
            }
            let setup = Setup {
                options,
                last,
                windows,
                geometry,
            };
            let (mut open, rx) = cx.update(|cx| this.open(frames, cx))?;
            let mut view = first;
            cx.update(|cx| open.show(view, &setup, cx));
            loop {
                let (index, event) = rx.recv().await.map_err(|_| Error::Cancelled)?;
                let Some(overlay) = open.overlays.get(index) else {
                    continue;
                };
                let (output, frame) = (overlay.output.clone(), overlay.frame.clone());
                match event {
                    Pick::Began => {}
                    Pick::Cancel => return Ok(Selection::Cancelled),
                    Pick::ToWindow | Pick::ToRegion => {
                        let next = if event == Pick::ToWindow {
                            View::Window
                        } else {
                            View::Region
                        };
                        if view != next && matches!(view, View::Region | View::Window) {
                            view = next;
                            cx.update(|cx| open.show(view, &setup, cx));
                        }
                    }
                    Pick::Color(c) => return Ok(Selection::Color(c)),
                    Pick::Region(local) if view == View::Scroll => {
                        drop(open);
                        let (surfaces, backend) = (this.surfaces.clone(), this.backend.clone());
                        let control = this.scroll.clone();
                        return scroll_flow::capture(surfaces, backend, control, output, local, cx)
                            .await;
                    }
                    Pick::Region(local) => {
                        let cropped = geometry::crop(&frame, local)
                            .ok_or_else(|| Error::Internal("empty selection".into()))?;
                        drop(open);
                        if view == View::Region {
                            let remember = this.remember.clone();
                            let last = remembered(&output, local);
                            cx.update(|cx| remember(last, cx));
                        }
                        return Ok(Selection::Region {
                            output,
                            logical: cropped.logical,
                            frame: Arc::new(cropped),
                        });
                    }
                    Pick::Window { id, local } => {
                        drop(open);
                        let backend = this.backend.clone();
                        let wanted = id.clone();
                        let captured = cx
                            .background_spawn(async move {
                                backend
                                    .capture
                                    .capture_window(&wanted, CaptureOpts { cursor: false })
                            })
                            .await;
                        let frame = window_frame(captured, &frame, local)?;
                        return Ok(Selection::Window {
                            id,
                            frame: Arc::new(frame),
                        });
                    }
                }
            }
        })
    }
}
