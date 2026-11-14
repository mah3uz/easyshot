use super::AgentCall;
use crate::run::{Env, launch};
use es_desktop::dbus::{Client, ClientError, Endpoint, Launch};
use es_protocol::JobStatus;
use es_protocol::agent::{AgentReply, ErrorCode, ReplyKind};
use std::ffi::OsString;
use std::future::Future;
use std::io::Write;

pub async fn run(
    call: &AgentCall,
    env: &Env,
    cancel: impl Future<Output = ()>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let reply = ask(call, env, cancel).await;
    print(call.json, &reply, out, err);
    i32::from(reply.exit_code())
}

async fn ask(call: &AgentCall, env: &Env, cancel: impl Future<Output = ()>) -> AgentReply {
    ask_via(call, &env.endpoint, launch(env), cancel).await
}

pub(super) async fn ask_via(
    call: &AgentCall,
    endpoint: &Endpoint,
    launch: Launch,
    cancel: impl Future<Output = ()>,
) -> AgentReply {
    let kind = call.route.kind();
    let client = match Client::connect(endpoint, launch).await {
        Ok(c) => c,
        Err(e) => return client_failure(kind, &e),
    };
    let (name, args) = (call.route.route_name(), call.route.args());
    if !call.wait {
        return match client.submit(name, &args).await {
            Ok(job) => AgentReply {
                job: Some(job),
                ..AgentReply::ok(kind)
            },
            Err(e) => client_failure(kind, &e),
        };
    }
    match client.run(name, &args, cancel).await {
        Ok(outcome) if outcome.status == JobStatus::Cancelled => {
            AgentReply::failed(kind, ErrorCode::Cancelled, "cancelled")
        }
        Ok(outcome) => AgentReply::from_json(&outcome.result).unwrap_or_else(|_| {
            AgentReply::failed(
                kind,
                ErrorCode::Failed,
                format!("the daemon's answer was not understood: {}", outcome.result),
            )
        }),
        Err(e) => client_failure(kind, &e),
    }
}

pub(super) fn client_failure(kind: ReplyKind, e: &ClientError) -> AgentReply {
    let (code, message) = match e {
        ClientError::DaemonUnavailable => (
            ErrorCode::NoDaemon,
            "the daemon is not running. Start it with `easyshot daemon` \
             or `systemctl --user start easyshot.service`."
                .to_owned(),
        ),
        ClientError::DaemonGone => (
            ErrorCode::NoDaemon,
            "the daemon went away before it finished".to_owned(),
        ),
        ClientError::Spawn(_) | ClientError::Bus(_) | ClientError::Fdo(_) => {
            (ErrorCode::NoDaemon, e.to_string())
        }
        ClientError::Rejected(reason) => (ErrorCode::InvalidArguments, reason.clone()),
        _ => (ErrorCode::Failed, e.to_string()),
    };
    let hint = (code == ErrorCode::NoDaemon).then(crate::hints::no_daemon);
    AgentReply::failed(kind, code, message).hinted(hint)
}

fn print(json: bool, reply: &AgentReply, out: &mut dyn Write, err: &mut dyn Write) {
    if json {
        let _ = writeln!(out, "{}", reply.to_json());
        return;
    }
    if let Some(e) = &reply.error {
        let _ = writeln!(err, "easyshot: {}", e.message);
        if let Some(hint) = &e.hint {
            let _ = writeln!(err, "{hint}");
        }
        return;
    }
    for o in reply.outputs.iter().flatten() {
        let focused = if o.focused == Some(true) {
            "  focused"
        } else {
            ""
        };
        let description = o.description.as_deref().map(clean).unwrap_or_default();
        let _ = writeln!(
            out,
            "{}\t{},{} {}x{}\tscale {}{focused}\t{description}",
            o.name, o.x, o.y, o.width, o.height, o.scale
        );
    }
    for w in reply.windows.iter().flatten() {
        let geometry = match (w.x, w.y, w.width, w.height) {
            (Some(x), Some(y), Some(width), Some(height)) => format!("{x},{y} {width}x{height}"),
            _ => "-".to_owned(),
        };
        let focused = match (w.focused == Some(true), w.visible) {
            (true, _) => "  focused",
            (false, false) => "  hidden",
            _ => "",
        };
        let _ = writeln!(
            out,
            "{}\t{}\t{geometry}\t{}{focused}\t{}",
            w.id,
            w.app_id.as_deref().map_or_else(|| "-".to_owned(), clean),
            w.output.as_deref().unwrap_or("-"),
            w.title.as_deref().map(clean).unwrap_or_default(),
        );
    }
    if let Some(shot) = &reply.capture {
        let _ = writeln!(out, "{}", shot.path);
    }
    if let Some(access) = &reply.access {
        super::access::print_status(access, out);
    }
    if let Some(config) = &reply.config {
        let _ = writeln!(out, "{}", config.value);
    }
    if let Some(done) = &reply.export {
        let _ = writeln!(out, "{}", done.path);
    }
    if let Some(rec) = &reply.recording {
        let shown = rec.path.as_ref().or(rec.id.as_ref()).or(rec.state.as_ref());
        if let Some(line) = shown {
            let _ = writeln!(out, "{line}");
        }
    }
    if let Some(ocr) = reply.ocr.as_deref().filter(|o| !o.text.is_empty()) {
        let _ = writeln!(out, "{}", ocr.text);
    }
    if let Some(job) = reply.job {
        let _ = writeln!(out, "job {job}");
    }
}

/// A window title may hold a tab or a newline, which would break the one-line-per-entry text.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Agent commands report a bad command line with their own exit status, and as JSON under `--json`.
pub fn usage_error(
    argv: &[OsString],
    e: &clap::Error,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    use clap::error::ErrorKind;
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        return None;
    }
    let first = argv
        .iter()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with('-'))?;
    if first != "agent" && first != "ai" {
        return None;
    }
    let rendered = e.render().to_string();
    let message = rendered
        .lines()
        .next()
        .unwrap_or_default()
        .trim_start_matches("error: ");
    let reply = AgentReply::failed(ReplyKind::Usage, ErrorCode::InvalidArguments, message);
    if argv.iter().any(|a| a == "--json") {
        let _ = writeln!(out, "{}", reply.to_json());
    } else {
        let _ = write!(err, "{rendered}");
    }
    Some(i32::from(reply.exit_code()))
}
