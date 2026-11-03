use crate::cli::Request;
use crate::doctor;
use anyhow::Result;
use es_desktop::dbus::{Client, ClientError, Endpoint, Launch};
use es_protocol::{JobStatus, Route};
use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub const EXIT_OK: i32 = 0;
pub const EXIT_ERROR: i32 = 1;
pub const EXIT_CANCELLED: i32 = 2;
pub const EXIT_NO_DAEMON: i32 = 3;

pub struct Env {
    pub endpoint: Endpoint,
    /// Start `easyshot daemon` when the bus cannot activate it.
    pub spawn_daemon: bool,
    pub wait: Duration,
    pub standalone_report: Box<dyn Fn() -> Result<String>>,
    pub log_dir: Option<PathBuf>,
    pub bundle_dir: PathBuf,
    pub now: u64,
}

pub fn execute(
    request: Request,
    env: &Env,
    cancel: impl Future<Output = ()>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    futures::executor::block_on(async {
        match request {
            Request::Route(route) => send(&route, env, cancel, out, err).await,
            Request::Doctor { json, bundle } => doctor(json, bundle, env, out, err).await,
            Request::Agent(call) => crate::agent::run(&call, env, cancel, out, err).await,
            Request::Skill(call) => crate::agent::run_skill(&call, out, err),
            Request::Annotate(call) => crate::agent::run_annotate(&call, out, err),
            Request::AgentMcp => crate::agent::serve_mcp(env, err),
            Request::Daemon | Request::Completions(_) => {
                let _ = writeln!(err, "easyshot: handled by main, not sent");
                EXIT_ERROR
            }
        }
    })
}

pub(crate) fn launch(env: &Env) -> Launch {
    launch_with(env.spawn_daemon, env.wait)
}

pub(crate) fn launch_with(spawn_daemon: bool, wait: Duration) -> Launch {
    let spawn = spawn_daemon.then(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("easyshot"));
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("daemon")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd
    });
    Launch { spawn, wait }
}

fn client_failure(e: &ClientError, err: &mut dyn Write) -> i32 {
    match e {
        ClientError::DaemonUnavailable => {
            let _ = writeln!(
                err,
                "easyshot: the daemon is not running. Start it with `easyshot daemon` \
                 or `systemctl --user start easyshot.service`."
            );
            EXIT_NO_DAEMON
        }
        ClientError::DaemonGone => {
            let _ = writeln!(err, "easyshot: the daemon went away before it finished.");
            EXIT_NO_DAEMON
        }
        ClientError::Bus(_) | ClientError::Fdo(_) => {
            let _ = writeln!(err, "easyshot: cannot talk to the session bus: {e}");
            EXIT_NO_DAEMON
        }
        _ => {
            let _ = writeln!(err, "easyshot: {e}");
            EXIT_ERROR
        }
    }
}

async fn send(
    route: &Route,
    env: &Env,
    cancel: impl Future<Output = ()>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let client = match Client::connect(&env.endpoint, launch(env)).await {
        Ok(c) => c,
        Err(e) => return client_failure(&e, err),
    };
    match client.run(&route.as_str(), &route.args(), cancel).await {
        Ok(outcome) => match outcome.status {
            JobStatus::Ok => {
                if !outcome.result.is_empty() {
                    let _ = writeln!(out, "{}", outcome.result);
                }
                EXIT_OK
            }
            JobStatus::Cancelled => {
                let _ = writeln!(err, "easyshot: cancelled");
                EXIT_CANCELLED
            }
            JobStatus::Error => {
                let _ = writeln!(err, "easyshot: {}", outcome.result);
                EXIT_ERROR
            }
        },
        Err(e) => client_failure(&e, err),
    }
}

async fn doctor(
    json: bool,
    bundle: bool,
    env: &Env,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let ask = Launch {
        spawn: None,
        wait: Duration::ZERO,
    };
    let from_daemon = match Client::connect(&env.endpoint, ask).await {
        Ok(client) => client.doctor().await.ok(),
        Err(_) => None,
    };
    let raw = match from_daemon {
        Some(raw) => raw,
        None => match (env.standalone_report)() {
            Ok(raw) => raw,
            Err(e) => {
                let _ = writeln!(err, "easyshot: the standalone probe failed: {e:#}");
                return EXIT_ERROR;
            }
        },
    };
    let shown = if json {
        format!("{}\n", doctor::pretty(&raw))
    } else {
        doctor::render_text(&raw)
    };
    let _ = out.write_all(shown.as_bytes());
    if bundle {
        match doctor::write_report_bundle(&raw, env.log_dir.as_deref(), &env.bundle_dir, env.now) {
            Ok(path) => {
                let _ = writeln!(err, "easyshot: wrote {}", path.display());
            }
            Err(e) => {
                let _ = writeln!(err, "easyshot: {e:#}");
                return EXIT_ERROR;
            }
        }
    }
    EXIT_OK
}
