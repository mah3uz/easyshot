#[path = "agent_capture.rs"]
pub(super) mod capture;
#[path = "agent_export.rs"]
mod export;
#[path = "agent_gate.rs"]
pub(super) mod gate;
#[path = "agent_hints.rs"]
mod hints;
#[path = "agent_ocr.rs"]
mod ocr;
#[path = "agent_record.rs"]
pub(super) mod record;
#[path = "agent_redact.rs"]
mod redact;

use es_app::dispatcher::AgentHandler;
use es_app::jobs::{Jobs, run_cpu};
use es_backend::{Backend, Caps};
use es_model::platform::{OutputInfo, WindowInfo};
use es_protocol::agent::{AgentReply, ErrorCode, OutputEntry, WindowEntry};
use es_protocol::{AgentRoute, JobStatus};
use gate::{Gate, Work};
use gpui::Task;
use std::rc::Rc;
use std::sync::Arc;

pub type OutputList = Arc<dyn Fn() -> Vec<OutputInfo> + Send + Sync>;
pub type WindowSnapshot = Arc<dyn Fn() -> Vec<WindowInfo> + Send + Sync>;

#[derive(Clone)]
pub struct Sources {
    pub caps: Caps,
    pub outputs: OutputList,
    pub windows: WindowSnapshot,
}

pub fn sources(backend: &Arc<Backend>) -> Sources {
    let (outputs, windows) = (backend.clone(), backend.clone());
    Sources {
        caps: backend.caps(),
        outputs: Arc::new(move || outputs.capture.outputs()),
        windows: Arc::new(move || windows.windows.snapshot()),
    }
}

#[cfg(test)]
pub fn handler(sources: Sources, jobs: Jobs, gate: Gate) -> AgentHandler {
    handler_with(sources, jobs, gate, None, None)
}

#[cfg(test)]
pub fn handler_with(
    sources: Sources,
    jobs: Jobs,
    gate: Gate,
    capture: Option<capture::CaptureRunner>,
    ocr: Option<ocr::OcrRunner>,
) -> AgentHandler {
    handler_full(sources, jobs, gate, capture, ocr, None)
}

pub fn handler_full(
    sources: Sources,
    jobs: Jobs,
    gate: Gate,
    capture: Option<capture::CaptureRunner>,
    ocr: Option<ocr::OcrRunner>,
    record: Option<record::RecordRunner>,
) -> AgentHandler {
    Rc::new(move |job, route, origin, cx| {
        let kind = route.kind();
        let work: Work = {
            let (sources, capture, ocr) = (sources.clone(), capture.clone(), ocr.clone());
            let record = record.clone();
            Box::new(move |route, scope, cx| match (route, capture, ocr) {
                (AgentRoute::Record(r), ..) => match record {
                    Some(run) => run(r.clone(), scope.clone(), cx),
                    None => Task::ready(AgentReply::failed(
                        kind,
                        ErrorCode::Unsupported,
                        "this desktop gives easyshot no screen recording",
                    )),
                },
                (AgentRoute::Capture(spec), Some(capture), _) => {
                    capture(spec.clone(), scope.clone(), cx)
                }
                (AgentRoute::Ocr(spec), _, Some(ocr)) => ocr(spec.clone(), scope.clone(), cx),
                _ => {
                    let route = route.clone();
                    let task = run_cpu(cx, move || answer(&route, &sources));
                    cx.background_executor().spawn(async move {
                        task.await.unwrap_or_else(|e| {
                            AgentReply::failed(kind, ErrorCode::Failed, e.to_string())
                        })
                    })
                }
            })
        };
        let reply = gate.pass(job, route, origin, &sources, work, cx);
        jobs.spawn_answer(job, cx, move |_| async move {
            let reply = reply.await;
            let status = if reply.ok {
                JobStatus::Ok
            } else {
                JobStatus::Error
            };
            (status, Some(reply.to_json()))
        });
    })
}

fn answer(route: &AgentRoute, sources: &Sources) -> AgentReply {
    let kind = route.kind();
    let unsupported = |what: &str| {
        AgentReply::failed(
            kind,
            ErrorCode::Unsupported,
            format!("this desktop gives easyshot no {what}"),
        )
    };
    match route {
        AgentRoute::ListOutputs => {
            if !sources.caps.contains(Caps::CAPTURE_OUTPUT) {
                return unsupported("output list");
            }
            AgentReply {
                outputs: Some((sources.outputs)().iter().map(output_entry).collect()),
                ..AgentReply::ok(kind)
            }
        }
        AgentRoute::Capture(_) => unsupported("screen capture"),
        AgentRoute::Export(spec) => export::export(spec),
        AgentRoute::Ocr(_) => unsupported("text recognition"),
        AgentRoute::Record(_) => unsupported("screen recording"),
        AgentRoute::ListWindows => {
            if !sources.caps.contains(Caps::WINDOW_LIST) {
                return unsupported("window list");
            }
            AgentReply {
                windows: Some((sources.windows)().iter().map(window_entry).collect()),
                ..AgentReply::ok(kind)
            }
        }
        AgentRoute::Manage(_) => {
            AgentReply::failed(kind, ErrorCode::Failed, "answered by the gate")
        }
    }
}

fn coord(v: f64) -> i32 {
    v.round() as i32
}

fn extent(v: f64) -> u32 {
    v.round().max(0.0) as u32
}

fn present(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

fn output_entry(o: &OutputInfo) -> OutputEntry {
    OutputEntry {
        name: o.id.0.clone(),
        description: present(&o.description),
        x: coord(o.logical.x0),
        y: coord(o.logical.y0),
        width: extent(o.logical.width()),
        height: extent(o.logical.height()),
        scale: o.scale,
        focused: o.focused,
    }
}

pub(super) fn window_entry(w: &WindowInfo) -> WindowEntry {
    let rect = w.logical;
    WindowEntry {
        id: w.id.0.clone(),
        app_id: present(&w.app_id),
        title: present(&w.title),
        x: rect.map(|r| coord(r.x0)),
        y: rect.map(|r| coord(r.y0)),
        width: rect.map(|r| extent(r.width())),
        height: rect.map(|r| extent(r.height())),
        output: w.output.as_ref().map(|o| o.0.clone()),
        focused: w.focus_rank.map(|rank| rank == 0),
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
