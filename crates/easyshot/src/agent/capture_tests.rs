use super::*;
use crate::agent::AgentCall;
use crate::cli::{Cli, Request};
use clap::Parser;
use es_protocol::AgentRoute;

fn parse(args: &[&str]) -> Result<AgentCall, clap::Error> {
    let cli = Cli::try_parse_from(std::iter::once("easyshot").chain(args.iter().copied()))?;
    match cli.request().unwrap() {
        Request::Agent(call) => Ok(call),
        other => panic!("not an agent call: {other:?}"),
    }
}

fn spec(args: &[&str]) -> CaptureSpec {
    match parse(args)
        .unwrap_or_else(|e| panic!("{args:?}: {e}"))
        .route
    {
        AgentRoute::Capture(spec) => spec,
        other => panic!("not a capture: {other:?}"),
    }
}

#[test]
fn each_target_flag_builds_its_typed_target() {
    use AgentTarget::*;
    assert_eq!(
        spec(&["agent", "capture", "--fullscreen"]).target,
        Fullscreen
    );
    assert_eq!(
        spec(&["ai", "capture", "--output", "DP-1"]).target,
        Output(OutputName::new("DP-1").unwrap())
    );
    assert_eq!(
        spec(&["ai", "capture", "--geometry", "-10,20 300x400"]).target,
        Geometry(es_protocol::agent::Geometry::parse("-10,20 300x400").unwrap())
    );
    assert_eq!(
        spec(&["ai", "capture", "--window", "foot"]).target,
        Window(WindowSelector::any("foot").unwrap())
    );
}

#[test]
fn exactly_one_target_is_needed() {
    for bad in [
        &["agent", "capture"][..],
        &["agent", "capture", "--fullscreen", "--output", "DP-1"],
        &["agent", "capture", "--window", "a", "--geometry", "0,0 1x1"],
        &["agent", "capture", "--delay", "3"],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn values_a_daemon_would_refuse_are_refused_at_the_command_line() {
    for bad in [
        &["agent", "capture", "--output", "-x"][..],
        &["agent", "capture", "--output", "a b"],
        &["agent", "capture", "--geometry", "10x10"],
        &["agent", "capture", "--geometry", "0,0 0x10"],
        &["agent", "capture", "--fullscreen", "--delay", "61"],
        &["agent", "capture", "--fullscreen", "--delay", "-1"],
        &[
            "agent",
            "capture",
            "--fullscreen",
            "--out",
            "/tmp/no/such/dir/a.png",
        ],
        &["agent", "capture", "--fullscreen", "--out", "/tmp/a.jpg"],
        &["agent", "capture", "--fullscreen", "--wait", "--no-wait"],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn options_reach_the_spec_and_a_relative_out_becomes_absolute() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("shot.png");
    let given = spec(&[
        "agent",
        "capture",
        "--fullscreen",
        "--out",
        out.to_str().unwrap(),
        "--delay",
        "60",
        "--ui",
    ]);
    assert_eq!(
        (given.out.as_deref(), given.delay, given.ui),
        (Some(out.as_path()), 60, true)
    );
    let relative = spec(&["agent", "capture", "--fullscreen", "--out", "rel.png"]);
    let path = relative.out.unwrap();
    assert!(path.is_absolute() && path.ends_with("rel.png"), "{path:?}");
    let plain = spec(&["agent", "capture", "--fullscreen"]);
    assert_eq!((plain.out, plain.delay, plain.ui), (None, 0, false));
}

#[test]
fn a_capture_is_quiet_unless_the_agent_asks_for_the_card() {
    assert!(!spec(&["agent", "capture", "--fullscreen"]).ui);
    assert!(spec(&["agent", "capture", "--fullscreen", "--ui"]).ui);
}

#[test]
fn the_old_no_ui_flag_still_parses_for_scripts_and_changes_nothing() {
    let old = spec(&["agent", "capture", "--fullscreen", "--no-ui"]);
    assert!(!old.ui);
    assert_eq!(old, spec(&["agent", "capture", "--fullscreen"]));
    assert!(parse(&["agent", "capture", "--fullscreen", "--ui", "--no-ui"]).is_err());
}

#[test]
fn a_capture_waits_unless_told_not_to() {
    let wait = |extra: &[&str]| {
        let mut args = vec!["agent", "capture", "--fullscreen"];
        args.extend_from_slice(extra);
        parse(&args).unwrap().wait
    };
    assert!(wait(&[]));
    assert!(wait(&["--wait"]));
    assert!(!wait(&["--no-wait"]));
}

#[test]
fn json_applies_to_a_capture_too() {
    assert!(
        parse(&["ai", "capture", "--fullscreen", "--json"])
            .unwrap()
            .json
    );
    assert!(
        parse(&["ai", "--json", "capture", "--fullscreen"])
            .unwrap()
            .json
    );
}

const SHOT: &str = r#"{"ok":true,"kind":"capture","path":"/tmp/a.png","width":640,"height":360,"scale":1.5,"output":"HEADLESS-1"}"#;

#[test]
fn a_capture_prints_its_path_or_exactly_the_json_answer() {
    use crate::run_tests::{PrivateBus, env, run_cli, serve};
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 2, es_protocol::JobStatus::Ok, SHOT);
    let (code, out, err) = run_cli(&["ai", "capture", "--fullscreen"], &env(&bus, dir.path()));
    assert_eq!((code, out.as_str(), err.as_str()), (0, "/tmp/a.png\n", ""));
    let (code, out, _) = run_cli(
        &["ai", "capture", "--fullscreen", "--json"],
        &env(&bus, dir.path()),
    );
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), 1);
    let json: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        json,
        serde_json::from_str::<serde_json::Value>(SHOT).unwrap()
    );
    let sent = server.finish();
    assert!(
        sent.iter()
            .all(|r| matches!(r, es_protocol::Route::Agent(AgentRoute::Capture(_)))),
        "{sent:?}"
    );
}
