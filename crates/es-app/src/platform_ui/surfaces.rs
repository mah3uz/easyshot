use super::{SurfaceHandle, SurfaceRole, Surfaces, layer};
use crate::errors::{Error, Result};
use es_backend::Backend;
use es_geom::{Point, Rect, Size};
use es_model::platform::{OutputId, OutputInfo};
use gpui::layer_shell::{Anchor, KeyboardInteractivity};
use gpui::{
    AnyView, AnyWindowHandle, App, AppContext as _, Bounds, Context, Edges, Entity, IntoElement,
    ParentElement as _, Pixels, Render, Styled as _, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point, px,
};
use gpui_component::Root;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

// A stale output scale must not leave a surface blank for good.
const GATE_LIMIT: Duration = Duration::from_millis(250);

pub trait OutputSource {
    fn outputs(&self) -> Vec<OutputInfo>;
    fn pointer_output(&self) -> Option<OutputId>;
    fn focused_output(&self) -> Option<OutputId>;
}

impl OutputSource for Backend {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.capture.outputs()
    }

    fn pointer_output(&self) -> Option<OutputId> {
        self.pointer.as_ref()?.output_under_pointer()
    }

    fn focused_output(&self) -> Option<OutputId> {
        if let Some(id) = self.pointer.as_ref().and_then(|p| p.focused_output()) {
            return Some(id);
        }
        let outputs = self.capture.outputs();
        outputs
            .iter()
            .find(|o| o.focused == Some(true))
            .or(outputs.first())
            .map(|o| o.id.clone())
    }
}

/// Shows nothing until the window's scale matches its output's, so the first visible frame is never resampled.
pub struct Gate {
    scale: f64,
    opened: Instant,
    content: Option<AnyView>,
}

impl Gate {
    pub fn new(scale: f64) -> Self {
        Gate {
            scale,
            opened: Instant::now(),
            content: None,
        }
    }

    pub fn ready(&self, window_scale: f32) -> bool {
        layer::scale_ready(window_scale, self.scale) || self.opened.elapsed() >= GATE_LIMIT
    }
}

impl Render for Gate {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full();
        if !self.ready(window.scale_factor()) {
            window.request_animation_frame();
            return root;
        }
        match &self.content {
            Some(content) => root.child(content.clone()),
            None => root,
        }
    }
}

pub struct LayerShellSurfaces {
    source: Arc<dyn OutputSource>,
    test_output: Option<OutputId>,
}

impl LayerShellSurfaces {
    pub fn new(source: Arc<dyn OutputSource>, test_output: Option<OutputId>) -> Self {
        LayerShellSurfaces {
            source,
            test_output,
        }
    }

    pub fn source(&self) -> &Arc<dyn OutputSource> {
        &self.source
    }

    pub fn test_output(&self) -> Option<&OutputId> {
        self.test_output.as_ref()
    }

    pub fn output_info(&self, output: &OutputId) -> Result<OutputInfo> {
        self.source
            .outputs()
            .into_iter()
            .find(|o| o.id == *output)
            .ok_or_else(|| Error::NotFound(format!("output {}", output.0)))
    }

    pub fn open_surface(
        &self,
        role: SurfaceRole,
        output: &OutputId,
        size: Size,
        cx: &mut App,
    ) -> Result<LayerSurfaceHandle> {
        let allowed = layer::check_test_output(self.test_output.as_ref(), output);
        debug_assert!(allowed.is_ok(), "{allowed:?}");
        allowed?;
        let info = self.output_info(output)?;
        let displays = cx.displays();
        let display =
            layer::find_display(displays.iter().map(|d| (d.id(), d.uuid().ok())), output)?;
        let pointer = self.source.pointer_output();
        let exclusive =
            layer::wants_exclusive(role, self.test_output.as_ref(), pointer.as_ref(), output);
        let options = layer::layer_options(role, exclusive);
        let anchor = options.anchor;
        let focus = options.keyboard_interactivity != KeyboardInteractivity::None;
        let gate = cx.new(|_| Gate::new(info.scale));
        let root_gate = gate.clone();
        let window = cx
            .open_window(
                WindowOptions {
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        logical_size(size),
                    ))),
                    app_id: Some(layer::namespace(role).into()),
                    window_background: WindowBackgroundAppearance::Transparent,
                    display_id: Some(display),
                    kind: WindowKind::LayerShell(options),
                    focus,
                    is_movable: false,
                    is_resizable: false,
                    ..Default::default()
                },
                move |window, cx| {
                    if layer::passes_input_through(role) {
                        window.set_input_region(Some(&[]));
                    }
                    // The theme plugin paints an opaque window background; layer surfaces show what is under them.
                    cx.new(|cx| Root::new(root_gate, window, cx).bg(gpui::transparent_black()))
                },
            )
            .map_err(|e| Error::Protocol(format!("cannot open {}: {e}", layer::namespace(role))))?;
        Ok(LayerSurfaceHandle {
            window,
            gate,
            output: info,
            anchor,
            size: Cell::new(size),
            placement: Rc::default(),
        })
    }

    /// The first surface pays the GPU device and pipeline set-up, so a daemon opens one before any capture.
    pub fn warm_up(&self, cx: &mut App) -> Result<()> {
        let output = match &self.test_output {
            Some(id) => id.clone(),
            None => self
                .source
                .outputs()
                .into_iter()
                .next()
                .map(|o| o.id)
                .ok_or_else(|| Error::NotFound("any output".into()))?,
        };
        let handle = self.open_surface(SurfaceRole::Countdown, &output, Size::new(1., 1.), cx)?;
        handle
            .window
            .update(cx, |_, window, _| {
                window.on_next_frame(|window, _| window.remove_window())
            })
            .map_err(|e| Error::Protocol(format!("warm-up surface: {e}")))
    }
}

impl Surfaces for LayerShellSurfaces {
    fn open(
        &self,
        role: SurfaceRole,
        output: &OutputId,
        size: Size,
        cx: &mut App,
    ) -> Result<Box<dyn SurfaceHandle>> {
        Ok(Box::new(self.open_surface(role, output, size, cx)?))
    }
}

#[derive(Default)]
struct Placement {
    pending: Cell<Option<Edges<Pixels>>>,
    armed: Cell<bool>,
}

pub struct LayerSurfaceHandle {
    window: WindowHandle<Root>,
    gate: Entity<Gate>,
    output: OutputInfo,
    anchor: Anchor,
    size: Cell<Size>,
    placement: Rc<Placement>,
}

impl LayerSurfaceHandle {
    pub fn window(&self) -> AnyWindowHandle {
        self.window.into()
    }

    pub fn output_info(&self) -> &OutputInfo {
        &self.output
    }

    pub fn size(&self) -> Size {
        self.size.get()
    }
}

// Applies the newest pending margins once per frame; motion between frames only replaces them.
fn release(placement: Rc<Placement>, window: &mut Window) {
    window.on_next_frame(move |window, _| match placement.pending.take() {
        Some(edges) => {
            window.set_layer_margin(edges);
            release(placement, window);
        }
        None => placement.armed.set(false),
    });
}

fn logical_size(size: Size) -> gpui::Size<Pixels> {
    gpui::size(px(size.width as f32), px(size.height as f32))
}

impl SurfaceHandle for LayerSurfaceHandle {
    fn set_content(&self, view: AnyView, cx: &mut App) {
        self.gate.update(cx, |gate, cx| {
            gate.content = Some(view);
            cx.notify();
        });
    }

    fn set_position(&self, global_logical: Point, cx: &mut App) {
        let edges = layer::margins(
            self.anchor,
            self.output.logical,
            self.size.get(),
            global_logical,
        );
        if self.placement.armed.get() {
            self.placement.pending.set(Some(edges));
            return;
        }
        self.placement.armed.set(true);
        let placement = self.placement.clone();
        let moved = self.window.update(cx, move |_, window, _| {
            window.set_layer_margin(edges);
            release(placement, window);
        });
        if moved.is_err() {
            self.placement.armed.set(false);
        }
    }

    fn resize(&self, size: Size, cx: &mut App) {
        self.size.set(size);
        let _ = self
            .window
            .update(cx, |_, window, _| window.resize(logical_size(size)));
    }

    fn set_input_region(&self, rects: &[Rect], cx: &mut App) {
        let region: Vec<Bounds<Pixels>> =
            rects.iter().map(|r| es_ui::convert::bounds(*r)).collect();
        let _ = self
            .window
            .update(cx, |_, window, _| window.set_input_region(Some(&region)));
    }

    fn output(&self) -> OutputId {
        self.output.id.clone()
    }

    fn close(&self, cx: &mut App) {
        let _ = self
            .window
            .update(cx, |_, window, _| window.remove_window());
    }
}
