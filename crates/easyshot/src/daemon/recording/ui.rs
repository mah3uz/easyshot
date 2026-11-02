use super::{Countdown, RecordingUi, Shown};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SurfaceHandle as _, SurfaceRole,
};
use es_desktop::RecordingTrayState;
use es_geom::{Point, Rect};
use es_model::platform::OutputId;
use es_overlays::countdown::{Clock, Countdown as CountdownView, CountdownStyle};
use es_overlays::record::{
    ControlBar, Dim, Placement, Recorded, control_bar, highlight_strips, tray_notice,
};
use gpui::{App, AppContext as _, Entity};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const UNMAP_SETTLE: Duration = Duration::from_millis(150);

pub type TrayUpdate = Rc<dyn Fn(Option<RecordingTrayState>, &mut App)>;
pub type Notify = Rc<dyn Fn(String)>;

fn open(
    surfaces: &LayerShellSurfaces,
    role: SurfaceRole,
    output: &OutputId,
    rect: Rect,
    cx: &mut App,
) -> Result<LayerSurfaceHandle> {
    let handle = surfaces.open_surface(role, output, rect.size(), cx)?;
    handle.set_position(Point::new(rect.x0, rect.y0), cx);
    Ok(handle)
}

pub struct LayerUi {
    pub surfaces: Rc<LayerShellSurfaces>,
    pub tray: TrayUpdate,
    pub notify: Notify,
}

struct LayerShown {
    dim: Vec<LayerSurfaceHandle>,
    bar: Option<(LayerSurfaceHandle, Cell<Rect>)>,
}

impl Shown for LayerShown {
    fn resized(&self, bar: &Entity<ControlBar>, cx: &mut App) {
        let Some((handle, rect)) = &self.bar else {
            return;
        };
        let size = bar.read(cx).size();
        let old = rect.get();
        let moved = Rect::from_origin_size(
            Point::new(
                old.x0 + (old.width() - size.width) / 2.0,
                old.y1 - size.height,
            ),
            (size.width, size.height),
        );
        rect.set(moved);
        handle.resize(size, cx);
        handle.set_position(Point::new(moved.x0, moved.y0), cx);
    }

    fn close(&self, cx: &mut App) {
        for handle in &self.dim {
            handle.close(cx);
        }
        if let Some((handle, _)) = &self.bar {
            handle.close(cx);
        }
    }
}

impl RecordingUi for LayerUi {
    fn show(&self, recorded: &Recorded, bar: &Entity<ControlBar>, cx: &mut App) -> Box<dyn Shown> {
        let outputs = self.surfaces.source().outputs();
        let mut shown = LayerShown {
            dim: Vec::new(),
            bar: None,
        };
        if let Recorded::Area(area) = recorded {
            let main = outputs.iter().max_by(|a, b| {
                let (x, y) = (a.logical.intersect(*area), b.logical.intersect(*area));
                (x.width().max(0.) * x.height().max(0.))
                    .total_cmp(&(y.width().max(0.) * y.height().max(0.)))
            });
            if let Some(out) = main {
                for strip in highlight_strips(*area, out.logical) {
                    match open(&self.surfaces, SurfaceRole::Highlight, &out.id, strip, cx) {
                        Ok(handle) => {
                            handle.set_input_region(&[], cx);
                            handle.set_content(cx.new(|_| Dim).into(), cx);
                            shown.dim.push(handle);
                        }
                        Err(e) => tracing::warn!(error = %e, "area highlight"),
                    }
                }
            }
        }
        let size = bar.read(cx).size();
        match control_bar(recorded, &outputs, size) {
            Placement::On { output, origin } => {
                let rect = Rect::from_origin_size(origin, (size.width, size.height));
                match open(&self.surfaces, SurfaceRole::Bar, &output, rect, cx) {
                    Ok(handle) => {
                        handle.set_content(bar.clone().into(), cx);
                        tracing::info!(
                            top = rect.y0,
                            bottom = rect.y1,
                            "recording control bar placed"
                        );
                        shown.bar = Some((handle, Cell::new(rect)));
                    }
                    Err(e) => tracing::warn!(error = %e, "control bar"),
                }
            }
            Placement::Tray => (self.notify)(tray_notice("easyshot record stop")),
        }
        Box::new(shown)
    }

    fn tray(&self, state: Option<RecordingTrayState>, cx: &mut App) {
        (self.tray)(state, cx);
    }
}

fn pointer_output(surfaces: &LayerShellSurfaces) -> Option<OutputId> {
    let source = surfaces.source();
    surfaces
        .test_output()
        .cloned()
        .or_else(|| source.pointer_output())
        .or_else(|| source.focused_output())
        .or_else(|| source.outputs().first().map(|o| o.id.clone()))
}

/// The recording countdown, a centred number on the pointer's output that takes no input.
pub fn countdown(surfaces: Rc<LayerShellSurfaces>) -> Countdown {
    Rc::new(move |secs, cx| {
        let Some(output) = pointer_output(&surfaces) else {
            return gpui::Task::ready(Err(Error::NotFound("an output for the countdown".into())));
        };
        let opened = surfaces.output_info(&output).and_then(|info| {
            surfaces.open_surface(SurfaceRole::Countdown, &output, info.logical.size(), cx)
        });
        let handle = match opened {
            Ok(handle) => handle,
            Err(e) => return gpui::Task::ready(Err(e)),
        };
        handle.set_input_region(&[], cx);
        let view = cx.new(|_| CountdownView::new(CountdownStyle::Recording, secs, Clock::real()));
        handle.set_content(view.clone().into(), cx);
        let run = CountdownView::run(&view, cx);
        cx.spawn(async move |cx| {
            run.await;
            cx.update(|cx| handle.close(cx));
            cx.background_executor().timer(UNMAP_SETTLE).await;
            Ok(())
        })
    })
}

#[cfg(feature = "player")]
pub fn open_player(video: std::path::PathBuf, paths: es_store::Paths, cx: &mut App) -> Result<()> {
    use es_overlays::player::{AudioOut, PlayerConfig, PlayerWindow, SystemFfmpeg};
    use std::sync::Arc;
    let output = video.with_file_name("trimmed.mp4");
    let config = PlayerConfig {
        video,
        output,
        audio: AudioOut::Speakers,
        paths,
        commands: Arc::new(es_record::project::SystemRunner),
        ffmpeg: Arc::new(SystemFfmpeg),
    };
    cx.open_window(gpui::WindowOptions::default(), |_, cx| {
        cx.new(|cx| PlayerWindow::new(config, cx))
    })
    .map(|_| ())
    .map_err(|e| Error::Protocol(e.to_string()))
}
