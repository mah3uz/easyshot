use super::color::{ColorEvent, ColorPicker};
use super::geometry;
use super::live::{self, Listening, Slot};
use super::picker::WindowPicker;
use super::scroll_flow::{self, Control};
use super::selector::{RegionSelector, SelectorEvent, SelectorStyle};
use super::window::{PickEvent, WindowSelector, window_frame};
use super::windows;
use es_app::capture_coordinator::Hook;
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SelectMode, Selection, SelectionFlow, SurfaceHandle,
    SurfaceRole, Switch,
};
use es_backend::Backend;
use es_geom::{Rect, Rgba};
use es_model::LastRegion;
use es_model::platform::{CaptureOpts, Frame, OutputId, WindowId, WindowInfo};
use gpui::{AnyView, App, AppContext as _, AsyncApp, Entity, Subscription, Task};
use std::cell::Cell;
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
    picking: Slot<(usize, Pick)>,
    showing: Rc<Cell<Option<View>>>,
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
            picking: Slot::default(),
            showing: Rc::default(),
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
            picking: self.picking.clone(),
            showing: self.showing.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Region,
    Text,
    Window,
    Color,
    ScrollWindow,
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
    Switch(View),
    Whole,
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
    tx: async_channel::Sender<(u64, (usize, Pick))>,
    listening: Listening<(usize, Pick)>,
    showing: Rc<Cell<Option<View>>>,
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
            View::Window | View::ScrollWindow if !setup.geometry => {
                let items = (index == 0).then(|| windows::listed(&setup.windows, pid));
                Content::Picker(cx.new(|cx| WindowPicker::new(&frame, items, cx)))
            }
            View::Window | View::ScrollWindow => {
                let here = windows::on_output(&setup.windows, &o.output, o.logical, pid);
                Content::Window(cx.new(|cx| WindowSelector::new(frame, here, cx)))
            }
            View::Color => Content::Color(cx.new(|cx| ColorPicker::new(frame, cx))),
        })
    }

    fn show(&mut self, view: View, setup: &Setup, cx: &mut App) {
        self.showing.set(Some(view));
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
        let stamp = self.listening.stamp();
        for (index, o) in self.overlays.iter().enumerate() {
            let (tx, stamp) = (self.tx.clone(), stamp.clone());
            let send = move |pick: Pick| {
                let _ = tx.try_send((stamp(), (index, pick)));
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
        tx: async_channel::Sender<(u64, (usize, Pick))>,
        listening: Listening<(usize, Pick)>,
        cx: &mut App,
    ) -> Result<Open> {
        let mut open = Open {
            overlays: Vec::new(),
            subscriptions: Vec::new(),
            tx,
            listening,
            showing: self.showing.clone(),
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
        Ok(open)
    }
}

fn view_for(mode: SelectMode) -> Option<View> {
    match mode {
        SelectMode::Region => Some(View::Region),
        SelectMode::Text => Some(View::Text),
        SelectMode::Window => Some(View::Window),
        SelectMode::Color => Some(View::Color),
        SelectMode::Scroll => Some(View::ScrollWindow),
        SelectMode::ScrollRegion => Some(View::Scroll),
    }
}

/// Space swaps between picking a window and drawing a region, within the same kind of capture.
fn toggled(view: View, to_window: bool) -> Option<View> {
    match (view, to_window) {
        (View::Region | View::Window, true) => Some(View::Window),
        (View::Region | View::Window, false) => Some(View::Region),
        (View::Scroll | View::ScrollWindow, true) => Some(View::ScrollWindow),
        (View::Scroll | View::ScrollWindow, false) => Some(View::Scroll),
        _ => None,
    }
}

/// Without any window to pick, a scroll capture starts on a region.
fn usable(view: View, setup: &Setup) -> View {
    if view == View::ScrollWindow && setup.windows.is_empty() && !setup.geometry {
        View::Scroll
    } else {
        view
    }
}

fn mode_of(view: View) -> SelectMode {
    match view {
        View::Region => SelectMode::Region,
        View::Text => SelectMode::Text,
        View::Window => SelectMode::Window,
        View::Color => SelectMode::Color,
        View::Scroll => SelectMode::Scroll,
    }
}

impl SelectionFlow for LayerShellSelectionFlow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        let Some(first) = view_for(mode) else {
            return Task::ready(Err(Error::Unsupported));
        };
        self.showing.set(Some(first));
        let ids: Vec<OutputId> = self
            .surfaces
            .source()
            .outputs()
            .into_iter()
            .map(|o| o.id)
            .collect();
        let backend = self.backend.clone();
        let (tx, rx) = async_channel::unbounded();
        let listening = Listening::start(&self.picking, tx.clone());
        let frozen = cx.background_spawn(async move {
            let frames = backend
                .capture
                .capture_outputs(&ids, CaptureOpts { cursor: false });
            let windows = (backend.windows.snapshot(), backend.windows.has_geometry());
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
            let mut open = cx.update(|cx| this.open(frames, tx, listening, cx))?;
            let mut view = usable(first, &setup);
            cx.update(|cx| open.show(view, &setup, cx));
            loop {
                let (tag, (index, event)) = rx.recv().await.map_err(|_| Error::Cancelled)?;
                if !open.listening.accept(tag) {
                    continue;
                }
                let Some(overlay) = open.overlays.get(index) else {
                    continue;
                };
                let (output, frame) = (overlay.output.clone(), overlay.frame.clone());
                match event {
                    Pick::Began => {}
                    Pick::Cancel => return Ok(Selection::Cancelled),
                    Pick::ToWindow | Pick::ToRegion => {
                        let next =
                            toggled(view, event == Pick::ToWindow).map(|v| usable(v, &setup));
                        if let Some(next) = next.filter(|next| *next != view) {
                            view = next;
                            cx.update(|cx| open.show(view, &setup, cx));
                        }
                    }
                    Pick::Switch(next) if usable(next, &setup) == view => {}
                    Pick::Switch(View::Window) if setup.windows.is_empty() && !setup.geometry => {}
                    Pick::Switch(next) => {
                        view = usable(next, &setup);
                        cx.update(|cx| open.show(view, &setup, cx));
                    }
                    Pick::Whole => {
                        drop(open);
                        return Ok(Selection::Region {
                            output,
                            logical: frame.logical,
                            frame,
                        });
                    }
                    Pick::Color(c) => return Ok(Selection::Color(c)),
                    Pick::Region(local) if view == View::Scroll => {
                        drop(open);
                        let (surfaces, backend) = (this.surfaces.clone(), this.backend.clone());
                        let control = this.scroll.clone();
                        return scroll_flow::capture(
                            surfaces, backend, control, output, local, None, cx,
                        )
                        .await;
                    }
                    Pick::Window { id, local } if view == View::ScrollWindow => {
                        let bounds =
                            Rect::new(0.0, 0.0, frame.logical.width(), frame.logical.height());
                        let area = local
                            .map(|l| l.intersect(bounds))
                            .filter(|a| a.width() >= 1.0 && a.height() >= 1.0);
                        let Some(area) = area else {
                            view = View::Scroll;
                            cx.update(|cx| open.show(view, &setup, cx));
                            continue;
                        };
                        drop(open);
                        let (surfaces, backend) = (this.surfaces.clone(), this.backend.clone());
                        let control = this.scroll.clone();
                        return scroll_flow::capture(
                            surfaces,
                            backend,
                            control,
                            output,
                            area,
                            Some(id),
                            cx,
                        )
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

    fn switch(&self, to: Switch, _cx: &mut App) -> bool {
        let pick = match to {
            Switch::To(mode) => view_for(mode).map_or(Pick::Cancel, Pick::Switch),
            Switch::Whole => Pick::Whole,
            Switch::Close => Pick::Cancel,
        };
        live::switch(&self.picking, (0, pick))
    }

    fn showing(&self) -> Option<SelectMode> {
        self.picking.borrow().as_ref()?;
        self.showing.get().map(mode_of)
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;

    #[test]
    fn a_scroll_capture_starts_on_a_window_and_the_region_start_is_separate() {
        assert_eq!(view_for(SelectMode::Scroll), Some(View::ScrollWindow));
        assert_eq!(view_for(SelectMode::ScrollRegion), Some(View::Scroll));
        assert_eq!(view_for(SelectMode::Window), Some(View::Window));
    }

    #[test]
    fn space_swaps_window_and_region_without_leaving_the_kind_of_capture() {
        assert_eq!(toggled(View::ScrollWindow, false), Some(View::Scroll));
        assert_eq!(toggled(View::Scroll, true), Some(View::ScrollWindow));
        assert_eq!(toggled(View::Window, false), Some(View::Region));
        assert_eq!(toggled(View::Region, true), Some(View::Window));
        assert_eq!(toggled(View::Color, true), None);
        assert_eq!(toggled(View::Text, false), None);
    }

    #[test]
    fn with_no_window_to_pick_a_scroll_capture_starts_on_a_region() {
        let mut setup = Setup {
            options: FlowOptions::default(),
            last: None,
            windows: Vec::new(),
            geometry: false,
        };
        assert_eq!(usable(View::ScrollWindow, &setup), View::Scroll);
        assert_eq!(usable(View::Window, &setup), View::Window);
        setup.geometry = true;
        assert_eq!(usable(View::ScrollWindow, &setup), View::ScrollWindow);
    }
}
