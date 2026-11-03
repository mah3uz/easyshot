#[cfg(any(test, feature = "test-hooks"))]
pub mod fake;
mod finish;
pub mod input;
mod target;
#[cfg(test)]
mod tests;
pub mod ui;

use es_app::capture_coordinator::CaptureKind;
use es_app::deck::{Deck, DeckItem};
use es_app::errors::{Error, Result};
use es_app::platform_ui::Selection;
use es_app::settings_model::SettingsModel;
use es_app::toast::{IconName, Toast, show_toast};
use es_desktop::RecordingTrayState;
use es_geom::Rect;
use es_model::CaptureKind as StoredKind;
use es_model::platform::{OutputId, OutputInfo};
use es_overlays::record::{
    ControlBar, ControlEvent, Phase, Recorded, RecordingRequest, RecordingTarget,
};
use es_protocol::Route;
use es_record::input::InputCollector;
use es_record::project::{CommandRunner, ProjectFolder};
use es_record::{Clock, RecordSpec, Recorder, RecorderEvent, RecordingHandle};
use es_store::Paths;
use gpui::{App, AppContext as _, Entity, Subscription, Task};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;

pub use ui::LayerUi;

const TRAY_TICK: Duration = Duration::from_secs(1);

/// The surfaces and tray state of a running recording.
pub trait Shown {
    fn resized(&self, bar: &Entity<ControlBar>, cx: &mut App);
    fn close(&self, cx: &mut App);
}

pub trait RecordingUi {
    fn show(
        &self,
        recorded: &Recorded,
        whole_screen_bar: bool,
        bar: &Entity<ControlBar>,
        cx: &mut App,
    ) -> Box<dyn Shown>;
    fn tray(&self, state: Option<RecordingTrayState>, cx: &mut App);
    fn open_trim(&self, video: std::path::PathBuf, template: String, cx: &mut App);
}

pub type RecorderFor = Rc<dyn Fn(&str) -> Result<Arc<dyn Recorder>>>;
pub type Countdown = Rc<dyn Fn(u32, &mut App) -> Task<Result<()>>>;
pub type AreaSelector = Rc<dyn Fn(&mut App) -> Task<Result<Selection>>>;

pub struct Deps {
    pub settings: Entity<SettingsModel>,
    pub paths: Paths,
    pub deck: Entity<Deck>,
    pub recorder: RecorderFor,
    pub recorder_missing: bool,
    pub outputs: Rc<dyn Fn() -> Vec<OutputInfo>>,
    pub ui: Rc<dyn RecordingUi>,
    pub clock: Arc<dyn Clock>,
    pub runner: Arc<dyn CommandRunner>,
    pub countdown: Countdown,
    pub select_area: AreaSelector,
    pub input: input::Inputs,
}

/// A request with its area chosen, in global logical points.
#[derive(Clone)]
pub struct Start {
    pub request: RecordingRequest,
    pub area: Option<(OutputId, Rect)>,
}

struct Session {
    start: Start,
    handle: Option<Box<dyn RecordingHandle>>,
    bar: Entity<ControlBar>,
    folder: ProjectFolder,
    shown: Box<dyn Shown>,
    recorder: &'static str,
    source: String,
    created_at: OffsetDateTime,
    input: Option<InputCollector>,
    _keystrokes_off: Subscription,
    _bar_events: Subscription,
    _events: Task<()>,
    _tick: Task<()>,
}

struct Inner {
    deps: Deps,
    active: RefCell<Option<Session>>,
    starting: Cell<bool>,
}

/// Held from the moment a start is asked for until the recorder runs or the start gives up.
struct Claim(Recordings);

impl Drop for Claim {
    fn drop(&mut self) {
        self.0.0.starting.set(false);
    }
}

#[derive(Clone)]
pub struct Recordings(Rc<Inner>);

fn io_error(e: &es_record::Error) -> Error {
    match e {
        es_record::Error::Unsupported => Error::Unsupported,
        other => Error::Protocol(other.to_string()),
    }
}

fn busy() -> Error {
    Error::Internal("the recording is busy".into())
}

/// A save notice that says where the file went and opens it when clicked.
fn saved_toast(title_id: &'static str, file: &std::path::Path) -> Toast {
    Toast::new(title_id, IconName::Check)
        .arg("path", file.display().to_string())
        .file(file.to_path_buf())
}

fn none_running() -> Error {
    Error::NotFound("a running recording".into())
}

use target::{output_name, resolve};

impl Recordings {
    pub fn new(deps: Deps) -> Self {
        Recordings(Rc::new(Inner {
            deps,
            active: RefCell::new(None),
            starting: Cell::new(false),
        }))
    }

    pub fn is_recording(&self) -> bool {
        self.0.active.borrow().is_some()
    }

    /// Recording, or between asking to start and the recorder running.
    pub fn is_busy(&self) -> bool {
        self.is_recording() || self.0.starting.get()
    }

    /// The one gate every start passes, before any selection or countdown shows.
    pub fn admit(&self, cx: &mut App) -> Result<()> {
        if self.0.deps.recorder_missing {
            show_toast(Toast::error("toast-recorder-missing"), cx);
            return Err(Error::Unsupported);
        }
        if self.is_busy() {
            show_toast(Toast::new("toast-recording-busy", IconName::Info), cx);
            return Err(busy());
        }
        Ok(())
    }

    fn claim(&self, cx: &mut App) -> Result<Claim> {
        self.admit(cx)?;
        self.0.starting.set(true);
        Ok(Claim(self.clone()))
    }

    /// The request a `record` route makes: the configured defaults for the chosen target.
    fn configured(&self, target: RecordingTarget, cx: &App) -> RecordingRequest {
        let cfg = &self.0.deps.settings.read(cx).config().recording;
        RecordingRequest {
            target,
            mic: (!cfg.mic.is_empty()).then(|| cfg.mic.clone()),
            system_audio: cfg.system_audio,
            delay_secs: cfg.start_delay,
        }
    }

    /// For the `record` and `record/area` routes, once the coordinator has the selection.
    pub fn with_selection(
        &self,
        kind: CaptureKind,
        selection: Selection,
        cx: &mut App,
    ) -> Task<Result<()>> {
        let Selection::Region {
            output, logical, ..
        } = selection
        else {
            return Task::ready(Err(Error::Cancelled));
        };
        let (target, area) = match kind {
            CaptureKind::RecordingArea => (RecordingTarget::Area, Some((output, logical))),
            _ => (RecordingTarget::Display(output), None),
        };
        let request = self.configured(target, cx);
        self.launch(Start { request, area }, cx)
    }

    /// From the capture bar: an area target is chosen first, then the delay runs, then recording starts.
    pub fn with_request(&self, request: RecordingRequest, cx: &mut App) -> Task<Result<()>> {
        let claim = match self.claim(cx) {
            Ok(claim) => claim,
            Err(e) => return Task::ready(Err(e)),
        };
        if !matches!(request.target, RecordingTarget::Area) {
            let start = Start {
                request,
                area: None,
            };
            return self.run(claim, start, cx);
        }
        let selected = (self.0.deps.select_area)(cx);
        let me = self.clone();
        cx.spawn(async move |cx| match selected.await? {
            Selection::Region {
                output, logical, ..
            } => {
                let area = Some((output, logical));
                cx.update(|cx| me.run(claim, Start { request, area }, cx))
                    .await
            }
            _ => Err(Error::Cancelled),
        })
    }

    pub fn launch(&self, start: Start, cx: &mut App) -> Task<Result<()>> {
        match self.claim(cx) {
            Ok(claim) => self.run(claim, start, cx),
            Err(e) => Task::ready(Err(e)),
        }
    }

    /// Runs the start delay, which is never part of the video, then starts.
    fn run(&self, claim: Claim, start: Start, cx: &mut App) -> Task<Result<()>> {
        let secs = start.request.delay_secs;
        let me = self.clone();
        let delay = (secs > 0).then(|| (self.0.deps.countdown)(secs, cx));
        cx.spawn(async move |cx| {
            if let Some(delay) = delay {
                delay.await?;
            }
            cx.update(|cx| me.install(claim, start, cx))
        })
    }

    pub fn start(&self, start: Start, cx: &mut App) -> Result<()> {
        let claim = self.claim(cx)?;
        self.install(claim, start, cx)
    }

    fn install(&self, claim: Claim, start: Start, cx: &mut App) -> Result<()> {
        let session = self.begin(start, cx)?;
        *self.0.active.borrow_mut() = Some(session);
        drop(claim);
        self.tick(cx);
        Ok(())
    }

    #[cfg(feature = "player")]
    pub fn trim(&self, video: std::path::PathBuf, cx: &mut App) -> Result<()> {
        let config = self.0.deps.settings.read(cx).config();
        let template = config.general.file_name_template.clone();
        ui::open_player(video, self.0.deps.paths.clone(), template, cx)
    }

    fn begin(&self, mut start: Start, cx: &mut App) -> Result<Session> {
        let deps = &self.0.deps;
        let outputs = (deps.outputs)();
        let (recorded, source) = resolve(&start, &outputs)?;
        let cfg = deps.settings.read(cx).config().recording.clone();
        let recorder = (deps.recorder)(&cfg.recorder)?;
        if start.request.system_audio
            && !recorder.can_record_system_audio(start.request.mic.is_some())
        {
            start.request.system_audio = false;
            show_toast(
                Toast::new("toast-recording-no-system-audio", IconName::Info),
                cx,
            );
        }
        let created_at = OffsetDateTime::now_utc();
        let folder = ProjectFolder::create(&deps.paths.recordings_dir(), created_at)
            .map_err(|e| io_error(&e))?;
        let spec = RecordSpec {
            source: source.clone(),
            fps: cfg.fps,
            codec: finish::codec(&cfg.codec),
            container: finish::container(&cfg.container),
            quality: finish::quality(&cfg.quality),
            system_audio: start.request.system_audio,
            mic: start.request.mic.clone(),
            cursor: true,
            dir: folder.dir().to_path_buf(),
        };
        recorder.prepare(&spec, &outputs);
        let handle = match recorder.start(spec) {
            Ok(handle) => handle,
            Err(e) => {
                let _ = std::fs::remove_dir_all(folder.dir());
                return Err(io_error(&e));
            }
        };
        let events = handle.events();
        let input = cfg
            .keystrokes
            .then(|| input::start(&deps.input, &recorded, &outputs, cx))
            .flatten();
        let me = self.clone();
        let keystrokes_off = cx.observe(&deps.settings, move |s, cx| {
            if !s.read(cx).config().recording.keystrokes {
                input::halt(me.take_input(), cx);
            }
        });
        let bar = cx.new(|cx| ControlBar::new(deps.clock.clone(), cx));
        let shown = deps.ui.show(&recorded, cfg.whole_screen_bar, &bar, cx);
        let me = self.clone();
        let bar_events = cx.subscribe(&bar, move |bar, event: &ControlEvent, cx| match event {
            ControlEvent::Route(route) => {
                if let Err(e) = me.route(route, cx) {
                    tracing::warn!(error = %e, "recording control");
                }
            }
            ControlEvent::Resized => {
                if let Some(s) = me.0.active.borrow().as_ref() {
                    s.shown.resized(&bar, cx);
                }
            }
        });
        let me = self.clone();
        let events_task = cx.spawn(async move |cx| {
            while let Ok(event) = events.recv().await {
                cx.update(|cx| me.on_event(event, cx));
            }
        });
        let me = self.clone();
        let tick = cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(TRAY_TICK).await;
                cx.update(|cx| me.tick(cx));
            }
        });
        Ok(Session {
            start,
            handle: Some(handle),
            bar,
            folder,
            shown,
            recorder: recorder.name(),
            source: output_name(&source),
            created_at,
            input,
            _keystrokes_off: keystrokes_off,
            _bar_events: bar_events,
            _events: events_task,
            _tick: tick,
        })
    }

    fn route(&self, route: &Route, cx: &mut App) -> Result<()> {
        match route {
            Route::RecordStop => self.stop(cx),
            Route::RecordPause => self.toggle_pause(cx),
            Route::RecordRestart => self.restart(cx),
            Route::RecordDiscard => self.discard(cx),
            _ => Ok(()),
        }
    }

    fn on_event(&self, event: RecorderEvent, cx: &mut App) {
        let Some(bar) = self.0.active.borrow().as_ref().map(|s| s.bar.clone()) else {
            return;
        };
        match event {
            RecorderEvent::Started => bar.update(cx, |b, cx| b.started(cx)),
            RecorderEvent::Paused => bar.update(cx, |b, cx| b.set_paused(true, cx)),
            RecorderEvent::Resumed => bar.update(cx, |b, cx| b.set_paused(false, cx)),
            RecorderEvent::Failed(message) => {
                tracing::error!(%message, "the recorder failed");
                show_toast(Toast::error("toast-error-generic"), cx);
                let _ = self.discard(cx);
                return;
            }
        }
        self.tick(cx);
    }

    fn tick(&self, cx: &mut App) {
        let state = self.0.active.borrow().as_ref().map(|s| {
            let bar = s.bar.read(cx);
            RecordingTrayState {
                elapsed: bar.elapsed(),
                paused: bar.phase() == Phase::Paused,
            }
        });
        self.0.deps.ui.tray(state, cx);
    }

    fn end_session(&self, cx: &mut App) -> Option<Session> {
        let session = self.0.active.borrow_mut().take()?;
        session.shown.close(cx);
        self.0.deps.ui.tray(None, cx);
        Some(session)
    }

    fn handle_of(&self) -> Result<(Box<dyn RecordingHandle>, Entity<ControlBar>)> {
        let mut active = self.0.active.borrow_mut();
        let session = active.as_mut().ok_or_else(none_running)?;
        let handle = session.handle.take().ok_or_else(busy)?;
        Ok((handle, session.bar.clone()))
    }

    pub fn toggle_pause(&self, cx: &mut App) -> Result<()> {
        let mut active = self.0.active.borrow_mut();
        let session = active.as_mut().ok_or_else(none_running)?;
        let paused = session.bar.read(cx).phase() == Phase::Paused;
        let handle = session.handle.as_mut().ok_or_else(busy)?;
        let done = if paused {
            handle.resume()
        } else {
            handle.pause()
        };
        done.map_err(|e| io_error(&e))
    }

    pub fn stop(&self, cx: &mut App) -> Result<()> {
        let (handle, bar) = self.handle_of()?;
        bar.update(cx, |b, cx| b.finishing(cx));
        let elapsed = bar.read(cx).elapsed();
        let collector = self.take_input();
        let read = cx
            .background_executor()
            .spawn(async move { collector.map(InputCollector::stop) });
        let me = self.clone();
        cx.spawn(async move |cx| {
            let out = handle.stop().await;
            let read = read.await;
            let deps = &me.0.deps;
            let done = {
                let active = me.0.active.borrow();
                let Some(s) = active.as_ref() else { return };
                let cfg = cx.update(|cx| deps.settings.read(cx).config().clone());
                (
                    ProjectFolder::open(s.folder.dir()),
                    finish::Meta {
                        elapsed,
                        fps: cfg.recording.fps,
                        codec: cfg.recording.codec.clone(),
                        container: cfg.recording.container.clone(),
                        recorder: s.recorder.to_owned(),
                        output: s.source.clone(),
                        system_audio: s.start.request.system_audio,
                        mic: s.start.request.mic.is_some(),
                        created_at: s.created_at,
                    },
                    matches!(
                        s.start.request.target,
                        RecordingTarget::Area | RecordingTarget::Window(_)
                    ),
                    cfg,
                )
            };
            let (folder, meta, partial, cfg) = done;
            let runner = deps.runner.clone();
            let kind = if partial {
                StoredKind::Region
            } else {
                StoredKind::Fullscreen
            };
            let completed = match out {
                Ok(out) => {
                    cx.background_executor()
                        .spawn(async move {
                            let input = finish::input(read, &out);
                            finish::complete(&folder, &out, &meta, &input, kind, runner.as_ref())
                        })
                        .await
                }
                Err(e) => Err(e),
            };
            cx.update(|cx| {
                me.end_session(cx);
                match completed {
                    Ok(done) => {
                        if cfg.recording.open_editor_after {
                            let template = cfg.general.file_name_template.clone();
                            deps.ui.open_trim(done.item.file.clone(), template, cx);
                        }
                        me.show_card(done.item, cfg.recording.save, cx)
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "recording not kept");
                        show_toast(Toast::error("toast-recording-save-failed"), cx);
                    }
                }
            });
        })
        .detach();
        Ok(())
    }

    fn show_card(&self, item: DeckItem, save: bool, cx: &mut App) {
        let id = item.id;
        tracing::info!(file = %item.file.display(), "recording card shown");
        self.0.deps.deck.update(cx, |d, cx| d.show(item, cx));
        if save {
            self.save_card(id, cx);
        }
    }

    pub fn discard(&self, cx: &mut App) -> Result<()> {
        self.drop_recording(false, cx)
    }

    pub fn restart(&self, cx: &mut App) -> Result<()> {
        self.drop_recording(true, cx)
    }

    fn take_input(&self) -> Option<InputCollector> {
        self.0.active.borrow_mut().as_mut()?.input.take()
    }

    fn drop_recording(&self, again: bool, cx: &mut App) -> Result<()> {
        let (handle, bar) = self.handle_of()?;
        input::halt(self.take_input(), cx);
        bar.update(cx, |b, cx| b.finishing(cx));
        let me = self.clone();
        cx.spawn(async move |cx| {
            if let Err(e) = handle.discard().await {
                tracing::warn!(error = %e, "recorder discard");
            }
            cx.update(|cx| {
                let Some(session) = me.end_session(cx) else {
                    return;
                };
                if let Err(e) = std::fs::remove_dir_all(session.folder.dir()) {
                    tracing::warn!(error = %e, "recording folder not removed");
                }
                if again && let Err(e) = me.start(session.start.clone(), cx) {
                    tracing::error!(error = %e, "start over");
                    show_toast(Toast::error("toast-error-generic"), cx);
                }
            });
        })
        .detach();
        Ok(())
    }

    /// Remuxes the card's recording into the save folder; the card goes when that works.
    pub fn save_card(&self, id: uuid::Uuid, cx: &mut App) {
        let deps = &self.0.deps;
        let Some(video) = deps.deck.read(cx).get(id).map(|i| i.file.clone()) else {
            return;
        };
        let config = deps.settings.read(cx).config().clone();
        let (runner, paths) = (deps.runner.clone(), deps.paths.clone());
        let deck = deps.deck.clone();
        cx.spawn(async move |cx| {
            let saved = cx
                .background_executor()
                .spawn(async move {
                    let now = OffsetDateTime::now_utc();
                    finish::save_mp4(&video, &paths, &config, now, runner.as_ref())
                })
                .await;
            cx.update(|cx| match saved {
                Ok(path) => {
                    tracing::info!(file = %path.display(), "recording saved");
                    show_toast(saved_toast("toast-recording-saved", &path), cx);
                    deck.update(cx, |d, cx| d.saved(id, cx));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "recording not saved");
                    show_toast(Toast::error("toast-recording-save-failed"), cx);
                    deck.update(cx, |d, cx| d.save_failed(id, cx));
                }
            });
        })
        .detach();
    }

    /// Cards for recordings that recovery finished at start-up.
    pub fn show_recovered(&self, found: &[es_record::recovery::Recovered], cx: &mut App) {
        self.0.deps.deck.update(cx, |deck, cx| {
            for r in found {
                deck.show(finish::recovered_card(r), cx);
            }
        });
    }
}
