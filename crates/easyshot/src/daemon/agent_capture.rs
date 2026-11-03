use super::gate::{self as agent_gate, scope::Scope};
use super::window_entry;
use crate::daemon::capture::{Direct, cut, shoot};
use es_app::agent_capture::{Options, Saved, save};
use es_app::capture_coordinator::{Direct as Source, DirectCapture};
use es_app::dispatcher::RouteDispatcher;
use es_app::errors::Error;
use es_app::jobs::Jobs;
use es_app::pipeline::PipelineDeps;
use es_app::platform_ui::LayerShellSurfaces;
use es_app::platform_ui::Selection;
use es_backend::{Backend, Caps, Capture, WindowList};
use es_geom::Rect;
use es_model::CaptureKind as StoredKind;
use es_model::LastTarget;
use es_model::platform::{Frame, OutputId, WindowInfo};
use es_protocol::agent::{
    AgentReply, AgentTarget, CaptureInfo, CaptureSpec, ErrorCode, ReplyKind, WindowSelector,
};
use gpui::{App, AsyncApp, Task};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub type CaptureRunner = Rc<dyn Fn(CaptureSpec, Scope, &mut App) -> Task<AgentReply>>;

pub struct Deps {
    pub backend: Arc<Backend>,
    pub direct: Rc<Direct>,
    pub pipeline: Rc<PipelineDeps>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Fail(pub ErrorCode, pub String);

impl Fail {
    pub(super) fn new(code: ErrorCode, message: impl Into<String>) -> Fail {
        Fail(code, message.into())
    }
}

pub(super) fn fail_of(e: Error) -> Fail {
    match e {
        Error::Unsupported => Fail::new(ErrorCode::Unsupported, "this desktop cannot capture that"),
        Error::NotFound(what) => Fail::new(ErrorCode::NotFound, format!("no {what}")),
        Error::Cancelled => Fail::new(ErrorCode::Cancelled, "cancelled"),
        Error::TesseractMissing => Fail::new(ErrorCode::Unsupported, super::ocr::INSTALL_HINT),
        other => Fail::new(ErrorCode::Failed, other.to_string()),
    }
}

pub(super) struct Shot {
    pub(super) frame: Arc<Frame>,
    pub(super) kind: StoredKind,
    pub(super) window: Option<WindowInfo>,
}

pub fn install(
    cx: &mut App,
    dispatcher: &RouteDispatcher,
    jobs: &Jobs,
    backend: &Arc<Backend>,
    surfaces: &Rc<LayerShellSurfaces>,
    pipeline: &Rc<PipelineDeps>,
) {
    let deps = Rc::new(Deps {
        backend: backend.clone(),
        direct: Rc::new(Direct::new(
            backend.clone(),
            surfaces.clone(),
            pipeline.paths.clone(),
        )),
        pipeline: pipeline.clone(),
    });
    let ocr = super::ocr::runner(deps.clone(), super::ocr::recognizer());
    dispatcher.set_agent_handler(super::handler_with(
        super::sources(backend),
        jobs.clone(),
        agent_gate::install(cx),
        Some(runner(deps)),
        Some(ocr),
    ));
}

fn runner(deps: Rc<Deps>) -> CaptureRunner {
    Rc::new(move |spec, scope, cx| {
        let deps = deps.clone();
        cx.spawn(async move |cx| {
            run(&deps, spec, &scope, cx)
                .await
                .unwrap_or_else(|Fail(code, message)| {
                    AgentReply::failed(ReplyKind::Capture, code, message)
                })
        })
    })
}

async fn run(
    deps: &Deps,
    spec: CaptureSpec,
    scope: &Scope,
    cx: &mut AsyncApp,
) -> Result<AgentReply, Fail> {
    if let Some(out) = &spec.out {
        check_out(out)?;
    }
    check_caps(deps.backend.caps, &spec.target)?;
    if spec.delay > 0 {
        cx.background_executor()
            .timer(Duration::from_secs(u64::from(spec.delay)))
            .await;
    }
    let shot = acquire(deps, &spec.target, scope, cx).await?;
    let options = Options {
        out: spec.out,
        ui: !spec.no_ui,
    };
    let saved = cx
        .update(|cx| save(&deps.pipeline, options, shot.kind, shot.frame, cx))
        .await
        .map_err(fail_of)?;
    Ok(reply(saved, shot.window.as_ref()))
}

fn reply(saved: Saved, window: Option<&WindowInfo>) -> AgentReply {
    AgentReply {
        capture: Some(Box::new(CaptureInfo {
            path: saved.path.display().to_string(),
            width: saved.width,
            height: saved.height,
            scale: saved.scale,
            output: saved
                .output
                .or_else(|| window.and_then(|w| w.output.as_ref().map(|o| o.0.clone()))),
            window: window.map(window_entry),
        })),
        ..AgentReply::ok(ReplyKind::Capture)
    }
}

fn check_out(out: &Path) -> Result<(), Fail> {
    let bad = |why: String| Fail::new(ErrorCode::InvalidArguments, why);
    match out.parent().map(Path::metadata) {
        Some(Ok(m)) if m.is_dir() => {}
        _ => {
            return Err(bad(format!(
                "the folder of {} does not exist",
                out.display()
            )));
        }
    }
    if out.is_dir() {
        return Err(bad(format!("{} is a folder", out.display())));
    }
    Ok(())
}

pub(super) fn check_caps(caps: Caps, target: &AgentTarget) -> Result<(), Fail> {
    let (need, what) = match target {
        AgentTarget::Window(_) => (Caps::WINDOW_LIST, "window list"),
        _ => (Caps::CAPTURE_OUTPUT, "screen capture"),
    };
    if caps.contains(need) {
        Ok(())
    } else {
        Err(Fail::new(
            ErrorCode::Unsupported,
            format!("this desktop gives easyshot no {what}"),
        ))
    }
}

pub(super) async fn acquire(
    deps: &Deps,
    target: &AgentTarget,
    scope: &Scope,
    cx: &mut AsyncApp,
) -> Result<Shot, Fail> {
    if matches!(target, AgentTarget::Fullscreen) {
        let picked = cx
            .update(|cx| deps.direct.run(Source::Fullscreen, cx))
            .await
            .map_err(fail_of)?;
        return match picked {
            Selection::Region { output, .. } if !scope.allows_output(&output.0) => {
                Err(outside(&format!("output {}", output.0)))
            }
            Selection::Region { frame, .. } => Ok(Shot {
                frame,
                kind: StoredKind::Fullscreen,
                window: None,
            }),
            _ => Err(Fail::new(ErrorCode::Failed, "nothing was captured")),
        };
    }
    let (backend, target, scope) = (deps.backend.clone(), target.clone(), scope.clone());
    cx.background_executor()
        .spawn(async move { grab(&*backend.capture, &*backend.windows, &target, &scope) })
        .await
}

pub(super) fn grab(
    capture: &dyn Capture,
    windows: &dyn WindowList,
    target: &AgentTarget,
    scope: &Scope,
) -> Result<Shot, Fail> {
    let outputs = capture.outputs();
    if !scope.is_open() && !matches!(target, AgentTarget::Window(_)) {
        scope
            .check(target, &outputs, &[])
            .map_err(|m| Fail::new(ErrorCode::AccessDenied, m))?;
    }
    let names = || {
        outputs
            .iter()
            .map(|o| o.id.0.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let crop =
        |output: &OutputId, area: Option<Rect>| cut(capture, output, area, None).map_err(fail_of);
    match target {
        AgentTarget::Fullscreen => Err(Fail::new(
            ErrorCode::Failed,
            "the focused output is chosen by the daemon",
        )),
        AgentTarget::Output(name) => {
            let output = outputs
                .iter()
                .find(|o| o.id.0 == name.as_str())
                .ok_or_else(|| {
                    Fail::new(
                        ErrorCode::NotFound,
                        format!("no output named {name}; the outputs are {}", names()),
                    )
                })?;
            Ok(Shot {
                frame: Arc::new(crop(&output.id, None)?),
                kind: StoredKind::Fullscreen,
                window: None,
            })
        }
        AgentTarget::Geometry(g) => {
            let (x, y) = (f64::from(g.x), f64::from(g.y));
            let area = Rect::new(x, y, x + f64::from(g.width), y + f64::from(g.height));
            let inside = |o: &es_model::platform::OutputInfo| {
                let l = o.logical;
                l.x0 <= area.x0 && l.y0 <= area.y0 && area.x1 <= l.x1 && area.y1 <= l.y1
            };
            let touches = |o: &es_model::platform::OutputInfo| {
                let l = o.logical;
                area.x0 < l.x1 && l.x0 < area.x1 && area.y0 < l.y1 && l.y0 < area.y1
            };
            let Some(output) = outputs.iter().find(|o| inside(o)) else {
                return Err(if outputs.iter().any(touches) {
                    Fail::new(
                        ErrorCode::InvalidArguments,
                        format!("{g} is not within one output; the outputs are {}", names()),
                    )
                } else {
                    Fail::new(
                        ErrorCode::NotFound,
                        format!("{g} is outside every output; the outputs are {}", names()),
                    )
                });
            };
            Ok(Shot {
                frame: Arc::new(crop(&output.id, Some(area))?),
                kind: StoredKind::Region,
                window: None,
            })
        }
        AgentTarget::Window(selector) => {
            let info = resolve_window(&windows.snapshot(), selector)?;
            if !scope.allows_window(&info) {
                return Err(outside(&format!("window {}", describe(&info))));
            }
            let last = LastTarget::Window {
                id: info.id.0.clone(),
                app: info.app_id.clone(),
                title: info.title.clone(),
            };
            let picked = shoot(capture, windows, last).map_err(fail_of)?;
            match picked {
                Some(Selection::Window { frame, .. }) => Ok(Shot {
                    frame,
                    kind: StoredKind::Window,
                    window: Some(info),
                }),
                _ => Err(Fail::new(
                    ErrorCode::NotFound,
                    format!("window {} closed before it was captured", info.id.0),
                )),
            }
        }
    }
}

fn outside(what: &str) -> Fail {
    Fail::new(
        ErrorCode::AccessDenied,
        format!(
            "{what} is outside what agents may capture; change it with `easyshot agent config set` or in Settings › Agents"
        ),
    )
}

fn describe(w: &WindowInfo) -> String {
    let clean = |s: &str| s.replace(|c: char| c.is_control(), " ");
    format!("{} ({}, {:?})", w.id.0, clean(&w.app_id), clean(&w.title))
}

pub(super) fn resolve_window(
    windows: &[WindowInfo],
    selector: &WindowSelector,
) -> Result<WindowInfo, Fail> {
    let by_id = |q: &str| windows.iter().filter(|w| w.id.0 == q).collect::<Vec<_>>();
    let by_app = |q: &str| windows.iter().filter(|w| w.app_id == q).collect::<Vec<_>>();
    let by_title = |q: &str| windows.iter().filter(|w| w.title == q).collect::<Vec<_>>();
    let (query, found) = match selector {
        WindowSelector::Id(q) => (q, by_id(q)),
        WindowSelector::AppId(q) => (q, by_app(q)),
        WindowSelector::Title(q) => (q, by_title(q)),
        WindowSelector::Any(q) => (
            q,
            [by_id(q), by_app(q), by_title(q)]
                .into_iter()
                .find(|f| !f.is_empty())
                .unwrap_or_default(),
        ),
    };
    match found.as_slice() {
        [] => Err(Fail::new(
            ErrorCode::NotFound,
            format!("no window matches {query:?}"),
        )),
        [one] => Ok((*one).clone()),
        many => Err(Fail::new(
            ErrorCode::InvalidArguments,
            format!(
                "{query:?} matches {} windows; name one by id: {}",
                many.len(),
                many.iter()
                    .map(|w| describe(w))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )),
    }
}

#[cfg(test)]
#[path = "agent_capture_tests.rs"]
mod tests;
