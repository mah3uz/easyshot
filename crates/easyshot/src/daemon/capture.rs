use es_app::capture_coordinator::{Direct as Source, DirectCapture};
use es_app::errors::{Error, Result};
use es_app::pipeline::Pinner;
use es_app::platform_ui::{ClipboardSink, LayerShellSurfaces, Selection};
use es_backend::Backend;
use es_geom::Rect;
use es_model::LastRegion;
use es_model::platform::{CaptureOpts, ClipItem, Frame, OutputId};
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

/// Fullscreen and previous-region captures, which need no selector.
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

fn grab(backend: &Backend, output: &OutputId) -> Result<Frame> {
    backend
        .capture
        .capture_outputs(std::slice::from_ref(output), CaptureOpts { cursor: false })
        .map_err(backend_error)?
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("a frame of {}", output.0)))
}

fn previous(backend: &Backend, last: &LastRegion) -> Result<Selection> {
    let output = OutputId(last.output.clone());
    let frame = grab(backend, &output)?;
    let local = Rect::new(
        f64::from(last.x),
        f64::from(last.y),
        f64::from(last.x) + f64::from(last.w),
        f64::from(last.y) + f64::from(last.h),
    );
    let cropped = geometry::crop(&frame, local)
        .ok_or_else(|| Error::NotFound("the previous region on its output".into()))?;
    Ok(Selection::Region {
        output,
        logical: cropped.logical,
        frame: Arc::new(cropped),
    })
}

impl DirectCapture for Direct {
    fn run(&self, source: Source, cx: &mut App) -> Task<Result<Selection>> {
        let backend = self.backend.clone();
        match source {
            Source::Fullscreen => {
                let Some(output) = self.output() else {
                    return Task::ready(Err(Error::NotFound("an output to capture".into())));
                };
                cx.background_executor().spawn(async move {
                    let frame = grab(&backend, &output)?;
                    Ok(Selection::Region {
                        output,
                        logical: frame.logical,
                        frame: Arc::new(frame),
                    })
                })
            }
            Source::Previous => {
                let last = es_store::state::open(&self.paths)
                    .ok()
                    .and_then(|s| s.value.last_region);
                let Some(last) = last else {
                    return Task::ready(Err(Error::NotFound("a previous region".into())));
                };
                cx.background_executor()
                    .spawn(async move { previous(&backend, &last) })
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
