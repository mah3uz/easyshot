use es_app::capture_coordinator::{CaptureKind, Captured, Direct as Source, DirectCapture};
use es_app::errors::{Error, Result};
use es_app::pipeline::Pinner;
use es_app::platform_ui::{ClipboardSink, LayerShellSurfaces, Selection};
use es_app::toast::{Toast, show_toast};
use es_backend::{Backend, Capture, WindowList};
use es_geom::Rect;
use es_model::platform::{CaptureOpts, ClipItem, Frame, OutputId, WindowId};
use es_model::{LastRegion, LastTarget};
use es_overlays::capture::flow::{Recall, Remember};
use es_overlays::capture::geometry;
use es_overlays::pin::{CopyImage, Pins};
use es_store::Paths;
use gpui::{App, Task};
use std::rc::Rc;
use std::sync::Arc;

pub fn recall(paths: Paths) -> Recall {
    Rc::new(move |_| es_store::state::open(&paths).ok()?.value.last_region)
}

pub fn remember(paths: Paths) -> Remember {
    Rc::new(move |region, _| {
        let saved = es_store::state::open(&paths).and_then(|mut state| {
            state.value.last_region = Some(region);
            state.save()
        });
        if let Err(e) = saved {
            tracing::warn!(error = %e, "previous region not remembered");
        }
    })
}

const NO_CURSOR: CaptureOpts = CaptureOpts { cursor: false };

/// Keeps what a region, window or fullscreen capture took, for `capture previous`.
pub fn remember_target(paths: Paths, backend: Arc<Backend>) -> Captured {
    Rc::new(move |kind, selection, _| {
        let target = match kind {
            CaptureKind::Fullscreen => match selection {
                Selection::Region { output, .. } => Some(LastTarget::Output(output.0.clone())),
                _ => None,
            },
            CaptureKind::Region | CaptureKind::Window => {
                target(selection, &*backend.capture, &*backend.windows)
            }
            _ => None,
        };
        let Some(target) = target else {
            return;
        };
        let saved = es_store::state::open(&paths).and_then(|mut state| {
            state.value.last_target = Some(target);
            state.save()
        });
        if let Err(e) = saved {
            tracing::warn!(error = %e, "capture target not remembered");
        }
    })
}

/// A region is kept relative to its output, so it follows the output if it moves.
fn target(
    selection: &Selection,
    capture: &dyn Capture,
    windows: &dyn WindowList,
) -> Option<LastTarget> {
    match selection {
        Selection::Region {
            output, logical, ..
        } => {
            let origin = capture
                .outputs()
                .into_iter()
                .find(|o| &o.id == output)?
                .logical;
            let r = logical.round();
            Some(LastTarget::Region(LastRegion {
                output: output.0.clone(),
                x: (r.x0 - origin.x0.round()) as i32,
                y: (r.y0 - origin.y0.round()) as i32,
                w: r.width().max(0.0) as u32,
                h: r.height().max(0.0) as u32,
            }))
        }
        Selection::Window { id, .. } => {
            let info = windows.snapshot().into_iter().find(|w| &w.id == id);
            let (app, title) = info.map(|w| (w.app_id, w.title)).unwrap_or_default();
            Some(LastTarget::Window {
                id: id.0.clone(),
                app,
                title,
            })
        }
        Selection::Color(_) | Selection::Cancelled => None,
    }
}

fn closed_toast(app: &str, title: &str) -> Toast {
    let name = if title.is_empty() { app } else { title };
    Toast::error("toast-window-closed").arg("window", name)
}

/// Fullscreen, previous and timed captures, which need no selector.
pub struct Direct {
    backend: Arc<Backend>,
    surfaces: Rc<LayerShellSurfaces>,
    paths: Paths,
}

impl Direct {
    pub fn new(backend: Arc<Backend>, surfaces: Rc<LayerShellSurfaces>, paths: Paths) -> Self {
        Direct {
            backend,
            surfaces,
            paths,
        }
    }

    fn output(&self) -> Option<OutputId> {
        let source = self.surfaces.source();
        self.surfaces
            .test_output()
            .cloned()
            .or_else(|| source.pointer_output())
            .or_else(|| source.focused_output())
    }
}

fn backend_error(e: es_backend::Error) -> Error {
    match e {
        es_backend::Error::Unsupported => Error::Unsupported,
        other => Error::Protocol(other.to_string()),
    }
}

fn grab(capture: &dyn Capture, output: &OutputId) -> Result<Frame> {
    capture
        .capture_outputs(std::slice::from_ref(output), NO_CURSOR)
        .map_err(backend_error)?
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("a frame of {}", output.0)))
}

fn cut(
    capture: &dyn Capture,
    output: &OutputId,
    global: Option<Rect>,
    local: Option<Rect>,
) -> Result<Frame> {
    let frame = grab(capture, &output)?;
    let local = match (local, global) {
        (Some(local), _) => local,
        (None, Some(g)) => g - frame.logical.origin().to_vec2(),
        (None, None) => return Ok(frame),
    };
    geometry::crop(&frame, local)
        .ok_or_else(|| Error::NotFound(format!("the area on {}", output.0)))
}

/// A fresh frame of `target`; `None` when it was a window that has closed since.
fn shoot(
    capture: &dyn Capture,
    windows: &dyn WindowList,
    target: LastTarget,
) -> Result<Option<Selection>> {
    let region = |output: OutputId, frame: Frame| Selection::Region {
        output,
        logical: frame.logical,
        frame: Arc::new(frame),
    };
    match target {
        LastTarget::Output(name) => {
            let output = OutputId(name);
            Ok(Some(region(
                output.clone(),
                cut(capture, &output, None, None)?,
            )))
        }
        LastTarget::Region(r) => {
            let output = OutputId(r.output);
            let (x, y) = (f64::from(r.x), f64::from(r.y));
            let local = Rect::new(x, y, x + f64::from(r.w), y + f64::from(r.h));
            Ok(Some(region(
                output.clone(),
                cut(capture, &output, None, Some(local))?,
            )))
        }
        LastTarget::Window { id, .. } => {
            let id = WindowId(id);
            let Some(info) = windows.snapshot().into_iter().find(|w| w.id == id) else {
                return Ok(None);
            };
            let frame = match capture.capture_window(&id, NO_CURSOR) {
                Ok(frame) => frame,
                Err(error) => {
                    let (Some(output), Some(global)) = (info.output, info.logical) else {
                        return Err(backend_error(error));
                    };
                    tracing::info!(%error, "window cut from its output");
                    cut(capture, &output, Some(global), None)?
                }
            };
            Ok(Some(Selection::Window {
                id,
                frame: Arc::new(frame),
            }))
        }
    }
}

impl Direct {
    fn take(&self, target: LastTarget, cx: &mut App) -> Task<Result<Selection>> {
        let backend = self.backend.clone();
        let closed = match &target {
            LastTarget::Window { app, title, .. } => Some(closed_toast(app, title)),
            _ => None,
        };
        let shot = cx
            .background_executor()
            .spawn(async move { shoot(&*backend.capture, &*backend.windows, target) });
        cx.spawn(async move |cx| match shot.await? {
            Some(selection) => Ok(selection),
            None => {
                if let Some(toast) = closed {
                    cx.update(|cx| show_toast(toast, cx));
                }
                Err(Error::Cancelled)
            }
        })
    }
}

impl DirectCapture for Direct {
    fn run(&self, source: Source, cx: &mut App) -> Task<Result<Selection>> {
        match source {
            Source::Fullscreen => {
                let Some(output) = self.output() else {
                    return Task::ready(Err(Error::NotFound("an output to capture".into())));
                };
                self.take(LastTarget::Output(output.0), cx)
            }
            Source::Previous => {
                let last = es_store::state::open(&self.paths)
                    .ok()
                    .and_then(|s| s.value.last_target);
                let Some(last) = last else {
                    return Task::ready(Err(Error::NotFound("a previous capture".into())));
                };
                self.take(last, cx)
            }
            Source::Again(picked) => {
                match target(&picked, &*self.backend.capture, &*self.backend.windows) {
                    Some(last) => self.take(last, cx),
                    None => Task::ready(Ok(picked)),
                }
            }
        }
    }
}

pub fn copy_file(clipboard: Rc<dyn ClipboardSink>) -> CopyImage {
    Rc::new(move |file, cx| {
        let png = match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(error = %e, "pinned image unreadable");
                return;
            }
        };
        let copied = clipboard.copy(vec![ClipItem::Png(png.into())], cx);
        cx.spawn(async move |_| {
            if let Err(e) = copied.await {
                tracing::warn!(error = %e, "copy from a pin");
            }
        })
        .detach();
    })
}

pub fn pinner(pins: Rc<Pins>) -> Pinner {
    Rc::new(move |file, _pixels, cx| {
        let image = es_overlays::deck::view::load_thumbnail(&file)
            .ok_or_else(|| Error::NotFound(file.display().to_string()))?;
        pins.pin(file, image, cx).map(|_| ())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_model::platform::{OutputEvent, OutputInfo, Transform, WindowEvent, WindowInfo};
    use std::sync::Mutex;
    use std::time::Instant;

    const SCREEN: Rect = Rect::new(1000., 0., 1400., 300.);

    fn dp1() -> OutputId {
        OutputId("DP-1".into())
    }

    fn frame(logical: Rect) -> Frame {
        let (w, h) = (logical.width() as u32, logical.height() as u32);
        Frame {
            output: Some(dp1()),
            size_px: (w, h),
            logical,
            scale: 1.0,
            bgra: vec![0; (w * h * 4) as usize].into(),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        }
    }

    #[derive(Default)]
    struct Shots {
        log: Mutex<Vec<String>>,
        window_alone: bool,
    }

    impl Shots {
        fn log(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }

    impl Capture for Shots {
        fn outputs(&self) -> Vec<OutputInfo> {
            vec![OutputInfo {
                id: dp1(),
                description: String::new(),
                logical: SCREEN,
                physical: (400, 300),
                scale: 1.0,
                transform: Transform::Normal,
                refresh_mhz: 60_000,
                focused: None,
            }]
        }

        fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
            async_channel::unbounded().1
        }

        fn capture_outputs(
            &self,
            ids: &[OutputId],
            _: CaptureOpts,
        ) -> es_backend::Result<Vec<Frame>> {
            self.log
                .lock()
                .unwrap()
                .push(format!("output {}", ids[0].0));
            Ok(vec![frame(SCREEN)])
        }

        fn capture_window(&self, w: &WindowId, _: CaptureOpts) -> es_backend::Result<Frame> {
            self.log.lock().unwrap().push(format!("window {}", w.0));
            if self.window_alone {
                Ok(frame(Rect::new(0., 0., 50., 40.)))
            } else {
                Err(es_backend::Error::Unsupported)
            }
        }
    }

    struct Windows(Vec<WindowInfo>);

    impl WindowList for Windows {
        fn snapshot(&self) -> Vec<WindowInfo> {
            self.0.clone()
        }

        fn has_geometry(&self) -> bool {
            true
        }

        fn events(&self) -> async_channel::Receiver<WindowEvent> {
            async_channel::unbounded().1
        }
    }

    fn terminal() -> WindowInfo {
        WindowInfo {
            id: WindowId("94".into()),
            app_id: "foot".into(),
            title: "~/src".into(),
            pid: None,
            logical: Some(Rect::new(1100., 50., 1300., 150.)),
            output: Some(dp1()),
            workspace: None,
            floating: false,
            focus_rank: None,
        }
    }

    fn terminal_target() -> LastTarget {
        LastTarget::Window {
            id: "94".into(),
            app: "foot".into(),
            title: "~/src".into(),
        }
    }

    #[test]
    fn a_closed_window_is_not_captured_and_its_toast_names_it() {
        let shots = Shots::default();
        let shot = shoot(&shots, &Windows(Vec::new()), terminal_target()).unwrap();
        assert!(shot.is_none());
        assert!(
            shots.log().is_empty(),
            "nothing captured: {:?}",
            shots.log()
        );
        let toast = closed_toast("foot", "~/src");
        assert_eq!(toast.title_id, "toast-window-closed");
        assert!(format!("{:?}", toast.message).contains("~/src"));
        assert!(format!("{:?}", closed_toast("foot", "").message).contains("foot"));
    }

    #[test]
    fn an_open_window_is_captured_again_by_its_id() {
        let shots = Shots {
            window_alone: true,
            ..Shots::default()
        };
        let shot = shoot(&shots, &Windows(vec![terminal()]), terminal_target()).unwrap();
        let Some(Selection::Window { id, frame }) = shot else {
            panic!("not a window shot");
        };
        assert_eq!((id.0.as_str(), frame.size_px), ("94", (50, 40)));
        assert_eq!(shots.log(), ["window 94"]);
    }

    #[test]
    fn a_window_that_cannot_be_captured_alone_is_cut_from_its_output() {
        let shots = Shots::default();
        let shot = shoot(&shots, &Windows(vec![terminal()]), terminal_target()).unwrap();
        let Some(Selection::Window { frame, .. }) = shot else {
            panic!("not a window shot");
        };
        assert_eq!(frame.logical, Rect::new(1100., 50., 1300., 150.));
        assert_eq!(shots.log(), ["window 94", "output DP-1"]);
    }

    #[test]
    fn a_region_is_kept_relative_to_its_output_and_taken_again_there() {
        let shots = Shots::default();
        let picked = Selection::Region {
            output: dp1(),
            logical: Rect::new(1100., 50., 1300., 150.),
            frame: Arc::new(frame(Rect::new(1100., 50., 1300., 150.))),
        };
        let windows = Windows(Vec::new());
        let kept = target(&picked, &shots, &windows).unwrap();
        let region = LastRegion {
            output: "DP-1".into(),
            x: 100,
            y: 50,
            w: 200,
            h: 100,
        };
        assert_eq!(kept, LastTarget::Region(region));
        let Some(Selection::Region { logical, .. }) = shoot(&shots, &windows, kept).unwrap() else {
            panic!("not a region shot");
        };
        assert_eq!(logical, Rect::new(1100., 50., 1300., 150.));
    }

    #[test]
    fn a_window_target_keeps_app_and_title_for_the_toast() {
        let picked = Selection::Window {
            id: WindowId("94".into()),
            frame: Arc::new(frame(Rect::new(0., 0., 50., 40.))),
        };
        let kept = target(&picked, &Shots::default(), &Windows(vec![terminal()]));
        assert_eq!(kept, Some(terminal_target()));
    }
}
