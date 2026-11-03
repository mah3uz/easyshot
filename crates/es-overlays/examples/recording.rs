use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, SurfaceHandle as _, SurfaceRole,
};
use es_backend::{Backend, Probe};
use es_geom::{Point, Rect, Size};
use es_model::platform::OutputInfo;
use es_overlays::record::bar::BAR;
use es_overlays::record::control::BAR as CONTROL;
use es_overlays::record::placement::home;
use es_overlays::record::{
    BarEvent, CaptureBar, ControlBar, ControlEvent, Dim, Placement, Recorded, control_bar,
    highlight_strips,
};
use es_protocol::Route;
use es_record::{
    Codec, Container, Quality, RecordSource, RecordSpec, Recorder, RecorderEvent, RecorderProbe,
    RecordingHandle, RecordingOutput, SystemClock,
};
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, QuitMode};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

struct FakeRecorder;

struct FakeHandle {
    dir: PathBuf,
    tx: async_channel::Sender<RecorderEvent>,
    rx: async_channel::Receiver<RecorderEvent>,
}

impl Recorder for FakeRecorder {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn probe(&self) -> RecorderProbe {
        RecorderProbe {
            available: true,
            reason: None,
        }
    }
    fn start(&self, spec: RecordSpec) -> es_record::Result<Box<dyn RecordingHandle>> {
        eprintln!("FAKE_RECORDER start {:?}", spec.source);
        let (tx, rx) = async_channel::unbounded();
        let _ = tx.try_send(RecorderEvent::Started);
        Ok(Box::new(FakeHandle {
            dir: spec.dir,
            tx,
            rx,
        }))
    }
}

impl RecordingHandle for FakeHandle {
    fn pause(&mut self) -> es_record::Result<()> {
        let _ = self.tx.try_send(RecorderEvent::Paused);
        Ok(())
    }
    fn resume(&mut self) -> es_record::Result<()> {
        let _ = self.tx.try_send(RecorderEvent::Resumed);
        Ok(())
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, es_record::Result<RecordingOutput>> {
        let file = self.dir.join("fake.mkv");
        Box::pin(async move {
            Ok(RecordingOutput {
                file,
                first_frame_monotonic_us: 0,
                segments: Vec::new(),
            })
        })
    }
    fn discard(self: Box<Self>) -> BoxFuture<'static, es_record::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.rx.clone()
    }
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("recording: {message}");
    std::process::exit(1);
}

fn grim(output: &str, path: &Path) {
    let ok = Command::new("grim")
        .args(["-o", output])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success());
    eprintln!(
        "SHOT {} {}",
        path.display(),
        if ok { "ok" } else { "failed" }
    );
}

fn press(backend: &Backend, at: Point) {
    if let Some(input) = backend.virtual_input.as_ref()
        && let Err(e) = input.scroll(at, 0.0)
    {
        eprintln!("virtual pointer: {e}");
    }
    thread::sleep(Duration::from_millis(60));
    for step in ["press", "release"] {
        let _ = Command::new("swaymsg")
            .args(["seat", "seat0", "cursor", step, "button1"])
            .status();
    }
}

struct Script {
    bar: Point,
    options_area: Point,
    control: Point,
}

// The bar and the options popover share one surface anchored to the bottom, so the bar keeps its spot.
fn drive(backend: &Backend, output: &str, shots: &Path, s: &Script) {
    let pause = |ms| thread::sleep(Duration::from_millis(ms));
    // A plain backdrop, so the dim around the area shows in the shots.
    let _ = Command::new("swaymsg")
        .args(["output", output, "bg", "#5A8FD8", "solid_color"])
        .status();
    pause(1500);
    grim(output, &shots.join("sway-capture-bar.png"));
    press(backend, at(s.bar, 590.0, 32.0));
    pause(800);
    grim(output, &shots.join("sway-recording-options.png"));
    press(backend, s.options_area);
    pause(2600);
    grim(output, &shots.join("sway-recording.png"));
    press(backend, at(s.control, 1.0 + 132.0 + 1.0 + 21.0, 19.0));
    pause(800);
    grim(output, &shots.join("sway-recording-paused.png"));
    press(backend, at(s.control, 60.0, 19.0));
}

fn at(p: Point, dx: f64, dy: f64) -> Point {
    Point::new(p.x + dx, p.y + dy)
}

fn open(
    surfaces: &LayerShellSurfaces,
    role: SurfaceRole,
    info: &OutputInfo,
    rect: Rect,
    cx: &mut App,
) -> LayerSurfaceHandle {
    let handle = surfaces
        .open_surface(role, &info.id, Size::new(rect.width(), rect.height()), cx)
        .unwrap_or_else(|e| fail(e));
    handle.set_position(Point::new(rect.x0, rect.y0), cx);
    handle
}

fn main() {
    let shots = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("usage: recording SCREENSHOT_DIR [below|above]"));
    let area = match std::env::args().nth(2).as_deref() {
        None | Some("below") => Rect::new(200.0, 150.0, 1080.0, 640.0),
        Some("above") => Rect::new(200.0, 300.0, 1080.0, 720.0),
        Some(other) => fail(format!("unknown placement {other}")),
    };
    let probe = Probe::gather().unwrap_or_else(|e| fail(e));
    let Some(test_output) = probe.test.as_ref().map(|t| t.output.clone()) else {
        fail("refusing to run outside test mode");
    };
    let backend = Arc::new(Backend::detect(&probe));
    let outputs = backend.capture.outputs();
    let Some(info) = outputs.iter().find(|o| o.id == test_output).cloned() else {
        fail("the test output is missing");
    };
    eprintln!(
        "OUTPUT {} logical={:?} scale={}",
        info.id.0, info.logical, info.scale
    );
    let area = area + Point::new(info.logical.x0, info.logical.y0).to_vec2();
    let Placement::On {
        origin: control, ..
    } = control_bar(&Recorded::Area(area), &outputs, CONTROL, true)
    else {
        fail("no room for the control bar");
    };
    let placed = Rect::from_origin_size(control, (CONTROL.width, CONTROL.height));
    let clear = placed.intersect(area);
    eprintln!("AREA {area:?} CONTROL {placed:?}");
    if clear.width() > 0.0 && clear.height() > 0.0 {
        fail("the control bar covers the recorded area");
    }
    let bar_home = home(info.logical, BAR);
    // Area is the first source button of the popover, which is centred over the bar.
    let area_x = info.logical.center().x - 230.0 + 60.5;
    let Some(input) = backend.virtual_input.as_ref() else {
        fail("no virtual pointer: run under tests/harness/sway.sh");
    };
    if let Err(e) = input.scroll(
        Point::new(info.logical.x0 + 10., info.logical.y0 + 10.),
        0.0,
    ) {
        fail(format!("virtual pointer: {e}"));
    }
    let (driver_backend, name, dir) = (backend.clone(), info.id.0.clone(), shots.clone());
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            es_ui::init(cx);
            let surfaces = Rc::new(LayerShellSurfaces::new(
                backend.clone(),
                Some(test_output.clone()),
            ));
            let bar_view = cx.new(|cx| CaptureBar::new(outputs.clone(), Vec::new(), false, cx));
            let bar_handle = open(
                &surfaces,
                SurfaceRole::Bar,
                &info,
                Rect::from_origin_size(bar_home, (BAR.width, BAR.height)),
                cx,
            );
            bar_handle.set_content(bar_view.clone().into(), cx);
            let options = bar_view.read(cx).options().read(cx).size();
            let script = Script {
                options_area: Point::new(area_x, bar_home.y - 8.0 - options.height + 97.0),
                bar: bar_home,
                control,
            };
            thread::spawn(move || drive(&driver_backend, &name, &dir, &script));
            let bar_handle = Rc::new(bar_handle);
            let (out, shots) = (info.clone(), shots.clone());
            cx.subscribe(&bar_view, move |bar, event: &BarEvent, cx| match event {
                BarEvent::Resized => {
                    let size = bar.read(cx).size(cx);
                    bar_handle.resize(size, cx);
                    bar_handle.set_position(home(out.logical, size), cx);
                }
                BarEvent::Record(request) => {
                    eprintln!("REQUEST {:?} delay={}", request.target, request.delay_secs);
                    bar_handle.close(cx);
                    let local = area - Point::new(out.logical.x0, out.logical.y0).to_vec2();
                    let source = request
                        .source(Some((out.id.clone(), local)))
                        .unwrap_or_else(|| fail("no source"));
                    record(&surfaces, &out, area, placed, source, &shots, cx);
                }
                other => eprintln!("BAR {other:?}"),
            })
            .detach();
        });
}

fn record(
    surfaces: &LayerShellSurfaces,
    out: &OutputInfo,
    area: Rect,
    placed: Rect,
    source: RecordSource,
    shots: &Path,
    cx: &mut App,
) {
    let mut handles = Vec::new();
    for strip in highlight_strips(area, out.logical) {
        let handle = open(surfaces, SurfaceRole::Highlight, out, strip, cx);
        handle.set_input_region(&[], cx);
        handle.set_content(cx.new(|_| Dim).into(), cx);
        handles.push(handle);
    }
    let control = cx.new(|cx| ControlBar::new(Arc::new(SystemClock::default()), cx));
    let handle = open(surfaces, SurfaceRole::Bar, out, placed, cx);
    handle.set_content(control.clone().into(), cx);
    handles.push(handle);
    let spec = RecordSpec {
        source,
        fps: 60,
        codec: Codec::H264,
        container: Container::Mkv,
        quality: Quality::High,
        system_audio: false,
        mic: None,
        cursor: true,
        dir: shots.to_path_buf(),
    };
    let mut recording = Some(FakeRecorder.start(spec).unwrap_or_else(|e| fail(e)));
    let events = recording.as_ref().map(|r| r.events());
    if let Some(events) = events {
        let control = control.clone();
        cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                eprintln!("RECORDER {event:?}");
                cx.update(|cx| {
                    control.update(cx, |bar, cx| match event {
                        RecorderEvent::Started => bar.started(cx),
                        RecorderEvent::Paused => bar.set_paused(true, cx),
                        RecorderEvent::Resumed => bar.set_paused(false, cx),
                        RecorderEvent::Failed(_) => {}
                    })
                });
            }
        })
        .detach();
    }
    cx.subscribe(&control, move |bar, event: &ControlEvent, cx| {
        match (event, &handles) {
            (ControlEvent::Route(Route::RecordPause), _) => {
                if let Some(r) = recording.as_mut() {
                    let paused = bar.read(cx).phase() == es_overlays::record::Phase::Paused;
                    let _ = if paused { r.resume() } else { r.pause() };
                }
            }
            (ControlEvent::Route(Route::RecordStop), _) => {
                bar.update(cx, |b, cx| b.finishing(cx));
                if let Some(r) = recording.take() {
                    cx.spawn(async move |cx| {
                        let out = r.stop().await;
                        eprintln!("STOPPED {:?}", out.map(|o| o.file));
                        cx.background_executor()
                            .timer(Duration::from_millis(500))
                            .await;
                        eprintln!("RECORDING_OK");
                        std::process::exit(0);
                    })
                    .detach();
                }
            }
            (other, _) => eprintln!("CONTROL {other:?}"),
        }
    })
    .detach();
}
