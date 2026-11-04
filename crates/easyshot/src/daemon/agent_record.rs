use super::Sources;
use super::capture::{Fail, check_caps, fail_of, hold, resolve_window};
use super::gate::scope::{Scope, describe};
use crate::daemon::recording::agent::{Ended, Kept, Options};
use crate::daemon::recording::{Recordings, Start};
use es_app::agent_access::{AccessEvent, AgentAccess};
use es_app::capture_coordinator::CaptureCoordinator;
use es_app::settings_model::SettingsModel;
use es_geom::Rect;
use es_model::platform::{OutputId, OutputInfo, WindowInfo};
use es_overlays::record::RecordingTarget;
use es_protocol::agent::{
    AgentReply, AgentTarget, ErrorCode, OutputName, RecordRoute, RecordSpec, RecordingInfo,
    ReplyKind,
};
use gpui::{App, Entity, Task};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

pub type RecordRunner = Rc<dyn Fn(RecordRoute, Scope, &mut App) -> Task<AgentReply>>;

const NONE_RUNNING: &str = "no recording started by an agent is running";
const BUSY: &str = "a recording is already running; stop it first";

enum Outcome {
    Stopped(RecordingInfo),
    Discarded,
    Lost(String),
}

struct Active {
    id: String,
    target: String,
    max_seconds: u32,
    stopping: bool,
    kept: Option<Kept>,
    waiters: Vec<async_channel::Sender<Outcome>>,
    _limit: Task<()>,
}

#[derive(Default)]
struct State {
    active: Option<Active>,
    last: Option<RecordingInfo>,
}

#[derive(Clone)]
struct Runner {
    recordings: Recordings,
    coordinator: CaptureCoordinator,
    sources: Sources,
    state: Rc<RefCell<State>>,
}

/// Recordings an agent starts, the limits that end them and the answers they give.
pub fn install(
    cx: &mut App,
    recordings: &Recordings,
    coordinator: &CaptureCoordinator,
    sources: Sources,
    access: &Entity<AgentAccess>,
    settings: &Entity<SettingsModel>,
) -> RecordRunner {
    let runner = Runner {
        recordings: recordings.clone(),
        coordinator: coordinator.clone(),
        sources,
        state: Rc::default(),
    };
    let state = runner.state.clone();
    recordings.observe(Rc::new(move |ended, _| ended_with(&state, ended)));
    let ending = runner.clone();
    cx.subscribe(access, move |_, event: &AccessEvent, cx| {
        if matches!(event, AccessEvent::GrantChanged(None)) {
            ending.end(cx);
        }
    })
    .detach();
    let ending = runner.clone();
    cx.observe(settings, move |settings, cx| {
        if !settings.read(cx).config().agent.enabled {
            ending.end(cx);
        }
    })
    .detach();
    Rc::new(move |route, scope, cx| runner.run(route, &scope, cx))
}

fn failed(code: ErrorCode, message: impl Into<String>) -> AgentReply {
    AgentReply::failed(ReplyKind::Recording, code, message)
}

fn reply(info: RecordingInfo) -> AgentReply {
    AgentReply {
        recording: Some(Box::new(info)),
        ..AgentReply::ok(ReplyKind::Recording)
    }
}

impl Runner {
    fn run(&self, route: RecordRoute, scope: &Scope, cx: &mut App) -> Task<AgentReply> {
        match route {
            RecordRoute::Start(spec) => self.start(spec, scope.clone(), cx),
            RecordRoute::Stop(id) => self.finish(id.as_deref(), false, cx),
            RecordRoute::Discard(id) => self.finish(id.as_deref(), true, cx),
            RecordRoute::Status => Task::ready(self.status(cx)),
        }
    }

    fn start(&self, spec: RecordSpec, scope: Scope, cx: &mut App) -> Task<AgentReply> {
        let me = self.clone();
        cx.spawn(async move |cx| {
            let sources = me.sources.clone();
            let seen = cx
                .background_executor()
                .spawn(async move { ((sources.outputs)(), (sources.windows)()) })
                .await;
            cx.update(|cx| me.begin(&spec, &scope, &seen.0, &seen.1, cx))
                .unwrap_or_else(|Fail(code, message)| failed(code, message))
        })
    }

    fn begin(
        &self,
        spec: &RecordSpec,
        scope: &Scope,
        outputs: &[OutputInfo],
        windows: &[WindowInfo],
        cx: &mut App,
    ) -> Result<AgentReply, Fail> {
        check_caps(self.sources.caps, &spec.target)?;
        let chosen = resolve(&spec.target, scope, outputs, windows)?;
        if self.recordings.is_busy() {
            return Err(Fail::new(ErrorCode::InvalidArguments, BUSY));
        }
        self.recordings
            .admit(cx)
            .map_err(|_| Fail::new(ErrorCode::Unsupported, "no screen recorder is installed"))?;
        let start = Start {
            request: self.recordings.agent_request(chosen.target, spec.audio, cx),
            area: chosen.area,
            options: Options {
                fps: spec.fps,
                keystrokes: spec.keystrokes,
            },
        };
        drop(hold(&self.coordinator)?);
        self.recordings.start(start, cx).map_err(fail_of)?;
        let id = format!("rec-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let (me, max) = (self.clone(), spec.max_seconds);
        let limit = cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_secs(u64::from(max)))
                .await;
            cx.update(|cx| me.end(cx));
        });
        self.state.borrow_mut().active = Some(Active {
            id: id.clone(),
            target: chosen.text.clone(),
            max_seconds: max,
            stopping: false,
            kept: None,
            waiters: Vec::new(),
            _limit: limit,
        });
        Ok(reply(RecordingInfo {
            id: Some(id),
            state: Some("recording".into()),
            target: Some(chosen.text),
            max_seconds: Some(max),
            ..RecordingInfo::default()
        }))
    }

    /// Stops the agent's recording, keeping its video; a recording nobody is waiting on is
    /// still answered later by `stop`.
    fn end(&self, cx: &mut App) {
        let first = match self.state.borrow_mut().active.as_mut() {
            Some(active) => !std::mem::replace(&mut active.stopping, true),
            None => false,
        };
        if first && let Err(e) = self.recordings.stop(cx) {
            tracing::warn!(error = %e, "agent recording not stopped");
        }
    }

    fn status(&self, cx: &App) -> AgentReply {
        let state = self.state.borrow();
        let info = match &state.active {
            Some(a) => RecordingInfo {
                id: Some(a.id.clone()),
                state: Some(if a.stopping { "stopping" } else { "recording" }.into()),
                target: Some(a.target.clone()),
                elapsed_ms: self
                    .recordings
                    .elapsed(cx)
                    .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX)),
                max_seconds: Some(a.max_seconds),
                ..RecordingInfo::default()
            },
            None => RecordingInfo {
                state: Some(
                    if self.recordings.is_recording() {
                        "recording"
                    } else {
                        "idle"
                    }
                    .into(),
                ),
                ..RecordingInfo::default()
            },
        };
        reply(info)
    }

    fn finish(&self, id: Option<&str>, discard: bool, cx: &mut App) -> Task<AgentReply> {
        let mut state = self.state.borrow_mut();
        let named = |have: &str| id.is_none_or(|id| id == have);
        let Some(active) = state.active.as_mut().filter(|a| named(&a.id)) else {
            let kept = state
                .last
                .clone()
                .filter(|l| !discard && named(l.id.as_deref().unwrap_or_default()));
            return Task::ready(match kept {
                Some(info) => reply(info),
                None => failed(ErrorCode::NotFound, NONE_RUNNING),
            });
        };
        let (tx, rx) = async_channel::bounded(1);
        active.waiters.push(tx);
        let first = !std::mem::replace(&mut active.stopping, true);
        let id = active.id.clone();
        drop(state);
        if first {
            let done = if discard {
                self.recordings.discard(cx)
            } else {
                self.recordings.stop(cx)
            };
            if let Err(e) = done {
                if let Some(a) = self.state.borrow_mut().active.as_mut() {
                    a.stopping = false;
                    a.waiters.clear();
                }
                return Task::ready(failed(ErrorCode::Failed, e.to_string()));
            }
        }
        cx.spawn(async move |_| match rx.recv().await {
            Ok(Outcome::Stopped(info)) if !discard => reply(info),
            Ok(Outcome::Discarded) if discard => reply(RecordingInfo {
                id: Some(id),
                state: Some("discarded".into()),
                ..RecordingInfo::default()
            }),
            Ok(Outcome::Discarded) => failed(ErrorCode::Cancelled, "the recording was discarded"),
            Ok(Outcome::Stopped(_)) => failed(
                ErrorCode::Failed,
                "the recording was stopped before it could be discarded",
            ),
            Ok(Outcome::Lost(message)) => failed(ErrorCode::Failed, message),
            Err(_) => failed(ErrorCode::Failed, "the recording ended without a result"),
        })
    }
}

fn ended_with(state: &RefCell<State>, ended: &Ended) {
    let mut state = state.borrow_mut();
    let Some(active) = state.active.as_mut() else {
        return;
    };
    let outcome = match ended {
        Ended::Kept(kept) if kept.saving => {
            active.kept = Some(kept.clone());
            return;
        }
        Ended::Kept(kept) => Outcome::Stopped(info_of(&active.id, kept, &kept.file)),
        Ended::Saved { card, path } => match active.kept.take().filter(|k| k.card == *card) {
            Some(kept) => Outcome::Stopped(info_of(&active.id, &kept, path)),
            None => return,
        },
        Ended::SaveFailed { card } => match active.kept.take().filter(|k| k.card == *card) {
            Some(kept) => Outcome::Stopped(info_of(&active.id, &kept, &kept.file)),
            None => return,
        },
        Ended::Discarded => Outcome::Discarded,
        Ended::Failed => Outcome::Lost("the recording could not be kept".into()),
    };
    let Some(done) = state.active.take() else {
        return;
    };
    if let Outcome::Stopped(info) = &outcome {
        state.last = Some(info.clone());
    }
    drop(state);
    for waiter in done.waiters {
        let _ = waiter.try_send(match &outcome {
            Outcome::Stopped(info) => Outcome::Stopped(info.clone()),
            Outcome::Discarded => Outcome::Discarded,
            Outcome::Lost(message) => Outcome::Lost(message.clone()),
        });
    }
}

fn info_of(id: &str, kept: &Kept, path: &Path) -> RecordingInfo {
    RecordingInfo {
        id: Some(id.to_owned()),
        path: Some(path.display().to_string()),
        duration_ms: Some(kept.duration_ms),
        width: Some(kept.width),
        height: Some(kept.height),
        bytes: Some(std::fs::metadata(path).map_or(0, |m| m.len())),
        ..RecordingInfo::default()
    }
}

#[derive(Debug)]
struct Chosen {
    target: RecordingTarget,
    area: Option<(OutputId, Rect)>,
    text: String,
}

/// Checks the scope against the output or window that is actually recorded, as a capture does.
fn resolve(
    target: &AgentTarget,
    scope: &Scope,
    outputs: &[OutputInfo],
    windows: &[WindowInfo],
) -> Result<Chosen, Fail> {
    let denied = |why: String| Fail::new(ErrorCode::AccessDenied, why);
    let names = || {
        outputs
            .iter()
            .map(|o| o.id.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let AgentTarget::Window(selector) = target {
        let info = resolve_window(windows, selector)?;
        if !scope.allows_window(&info) {
            return Err(denied(format!(
                "window {} is outside what agents may capture; change it with `easyshot agent config set` or in Settings › Agents",
                info.id.0
            )));
        }
        if info.logical.is_none() {
            return Err(Fail::new(
                ErrorCode::Unsupported,
                "this desktop does not tell easyshot where that window is",
            ));
        }
        let text = describe(target);
        return Ok(Chosen {
            target: RecordingTarget::Window(info),
            area: None,
            text,
        });
    }
    let no_such = |what: String| {
        Fail::new(
            ErrorCode::NotFound,
            format!("{what}; the outputs are {}", names()),
        )
    };
    let (output, checked, area) = match target {
        AgentTarget::Output(name) => {
            let o = outputs
                .iter()
                .find(|o| o.id.0 == name.as_str())
                .ok_or_else(|| no_such(format!("no output named {name}")))?;
            (o, target.clone(), None)
        }
        AgentTarget::Fullscreen => {
            let o = outputs
                .iter()
                .find(|o| o.focused == Some(true))
                .or(outputs.first())
                .ok_or_else(|| no_such("there is no output".into()))?;
            let checked =
                OutputName::new(&o.id.0).map_or(AgentTarget::Fullscreen, AgentTarget::Output);
            (o, checked, None)
        }
        AgentTarget::Geometry(g) => {
            let (x, y) = (f64::from(g.x), f64::from(g.y));
            let area = Rect::new(x, y, x + f64::from(g.width), y + f64::from(g.height));
            let inside = |o: &&OutputInfo| {
                let l = o.logical;
                l.x0 <= area.x0 && l.y0 <= area.y0 && area.x1 <= l.x1 && area.y1 <= l.y1
            };
            let touches = |o: &OutputInfo| o.logical.intersect(area).area() > 0.0;
            let Some(o) = outputs.iter().find(inside) else {
                return Err(if outputs.iter().any(touches) {
                    Fail::new(
                        ErrorCode::InvalidArguments,
                        format!("{g} is not within one output; the outputs are {}", names()),
                    )
                } else {
                    no_such(format!("{g} is outside every output"))
                });
            };
            (o, target.clone(), Some(area))
        }
        AgentTarget::Window(_) => return Err(Fail::new(ErrorCode::Failed, "unreachable")),
    };
    scope.check(&checked, outputs, windows).map_err(denied)?;
    let text = match target {
        AgentTarget::Fullscreen => format!("fullscreen ({})", output.id.0),
        other => describe(other),
    };
    Ok(match area {
        Some(area) => Chosen {
            target: RecordingTarget::Area,
            area: Some((output.id.clone(), area)),
            text,
        },
        None => Chosen {
            target: RecordingTarget::Display(output.id.clone()),
            area: None,
            text,
        },
    })
}

#[cfg(test)]
#[path = "agent_record_tests.rs"]
mod tests;
