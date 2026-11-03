use super::*;
use crate::cli::{Cli, Request};
use crate::run_tests::{PrivateBus, env, run_cli, serve};
use clap::Parser;
use es_desktop::dbus::ClientError;
use es_protocol::agent::{AgentReply, ErrorCode, ReplyKind};
use es_protocol::{JobStatus, Route};
use serde_json::{Value, json};
use std::ffi::OsString;

const OUTPUTS: &str = r#"{"ok":true,"kind":"outputs","outputs":[{"name":"HEADLESS-1","x":0,"y":0,"width":1280,"height":720,"scale":1.5,"focused":true}]}"#;
const WINDOWS: &str = r#"{"ok":true,"kind":"windows","windows":[{"id":"4","app_id":"foot","title":"a\tb","x":0,"y":0,"width":640,"height":360,"output":"HEADLESS-1","focused":true},{"id":"5"}]}"#;

fn call(args: &[&str]) -> AgentCall {
    let cli = Cli::try_parse_from(std::iter::once("easyshot").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("{args:?}: {e}"));
    match cli.request().unwrap() {
        Request::Agent(call) => call,
        other => panic!("not an agent call: {other:?}"),
    }
}

fn one_json_object(stdout: &str) -> Value {
    assert_eq!(stdout.lines().count(), 1, "exactly one line: {stdout:?}");
    serde_json::from_str(stdout).expect("a JSON object")
}

#[test]
fn agent_and_its_alias_build_the_same_typed_call() {
    let outputs = AgentCall {
        route: AgentRoute::ListOutputs,
        json: true,
        wait: true,
    };
    assert_eq!(call(&["agent", "list", "outputs", "--json"]), outputs);
    assert_eq!(call(&["ai", "list", "outputs", "--json"]), outputs);
    assert_eq!(call(&["agent", "--json", "list", "outputs"]), outputs);
    assert_eq!(
        call(&["ai", "list", "windows"]).route,
        AgentRoute::ListWindows
    );
    assert!(!call(&["agent", "list", "windows"]).json);
    for bad in [
        &["agent"][..],
        &["agent", "list"],
        &["agent", "list", "monitors"],
        &["ai", "capture"],
    ] {
        assert!(
            Cli::try_parse_from(std::iter::once("easyshot").chain(bad.iter().copied())).is_err(),
            "{bad:?}"
        );
    }
}

#[test]
fn list_json_prints_the_daemons_answer_as_one_object_and_exits_zero() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 2, JobStatus::Ok, OUTPUTS);
    let (code, out, err) = run_cli(
        &["agent", "list", "outputs", "--json"],
        &env(&bus, dir.path()),
    );
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(
        one_json_object(&out),
        serde_json::from_str::<Value>(OUTPUTS).unwrap()
    );
    let (code, out, _) = run_cli(&["ai", "list", "outputs"], &env(&bus, dir.path()));
    assert_eq!(code, 0);
    assert_eq!(out, "HEADLESS-1\t0,0 1280x720\tscale 1.5  focused\t\n");
    assert_eq!(
        server.finish(),
        vec![Route::Agent(AgentRoute::ListOutputs); 2]
    );
}

#[test]
fn window_text_keeps_one_line_per_window() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 1, JobStatus::Ok, WINDOWS);
    let (code, out, _) = run_cli(&["agent", "list", "windows"], &env(&bus, dir.path()));
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "4\tfoot\t0,0 640x360\tHEADLESS-1  focused\ta b\n5\t-\t-\t-\t\n"
    );
    server.finish();
}

#[test]
fn a_coded_failure_sets_its_exit_status_and_stays_json() {
    const DENIED: &str =
        r#"{"ok":false,"kind":"windows","error":{"code":"access_off","message":"off"}}"#;
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 1, JobStatus::Error, DENIED);
    let (code, out, err) = run_cli(
        &["agent", "list", "windows", "--json"],
        &env(&bus, dir.path()),
    );
    assert_eq!((code, err.as_str()), (7, ""));
    assert_eq!(one_json_object(&out)["error"]["code"], json!("access_off"));
    server.finish();
}

#[test]
fn cancellation_is_a_json_error_with_the_cancelled_status() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 1, JobStatus::Cancelled, "");
    let (code, out, _) = run_cli(
        &["agent", "list", "outputs", "--json"],
        &env(&bus, dir.path()),
    );
    assert_eq!(code, 2);
    assert_eq!(
        one_json_object(&out),
        json!({"ok": false, "kind": "outputs", "error": {"code": "cancelled", "message": "cancelled"}})
    );
    server.finish();
}

#[test]
fn no_daemon_is_a_json_error_with_exit_status_three() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = run_cli(
        &["agent", "list", "outputs", "--json"],
        &env(&bus, dir.path()),
    );
    assert_eq!((code, err.as_str()), (3, ""));
    assert_eq!(one_json_object(&out)["error"]["code"], json!("no_daemon"));
    let (code, out, err) = run_cli(&["agent", "list", "outputs"], &env(&bus, dir.path()));
    assert_eq!((code, out.as_str()), (3, ""));
    assert!(err.contains("not running"), "{err}");
}

#[test]
fn without_wait_the_command_returns_the_accepted_job() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 1, JobStatus::Ok, OUTPUTS);
    let accepted = AgentCall {
        route: AgentRoute::ListOutputs,
        json: true,
        wait: false,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = crate::run::execute(
        Request::Agent(accepted),
        &env(&bus, dir.path()),
        futures::future::pending(),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 0);
    let reply = one_json_object(&String::from_utf8(out).unwrap());
    assert_eq!(reply["ok"], json!(true));
    assert_eq!(reply["kind"], json!("outputs"));
    assert!(reply["job"].as_u64().is_some(), "{reply}");
    assert!(
        reply.get("outputs").is_none(),
        "the result is not waited for"
    );
    server.finish();
}

#[test]
fn a_refused_request_is_invalid_arguments() {
    let reply = run::client_failure(ReplyKind::Outputs, &ClientError::Rejected("bad".into()));
    assert_eq!(
        reply.error.map(|e| e.code),
        Some(ErrorCode::InvalidArguments)
    );
    let reply = run::client_failure(ReplyKind::Outputs, &ClientError::DaemonGone);
    assert_eq!(reply.exit_code(), 3);
}

#[test]
fn a_bad_agent_command_line_exits_four_and_is_json_under_json() {
    let parse = |words: &[&str]| {
        let argv: Vec<OsString> = std::iter::once("easyshot")
            .chain(words.iter().copied())
            .map(OsString::from)
            .collect();
        let e = Cli::try_parse_from(&argv).unwrap_err();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = usage_error(&argv, &e, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    };
    let (code, out, err) = parse(&["agent", "list", "monitors", "--json"]);
    assert_eq!((code, err.as_str()), (Some(4), ""));
    let reply = AgentReply::from_json(out.trim()).unwrap();
    assert_eq!(reply.kind, ReplyKind::Usage);
    assert_eq!(
        reply.error.map(|e| e.code),
        Some(ErrorCode::InvalidArguments)
    );
    let (code, out, err) = parse(&["ai", "list"]);
    assert_eq!((code, out.as_str()), (Some(4), ""));
    assert!(err.contains("Usage"), "{err}");
    assert_eq!(parse(&["capture", "bogus", "--json"]).0, None);
    assert_eq!(parse(&["agent", "--help"]).0, None);
}
