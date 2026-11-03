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

fn spec(args: &[&str]) -> ExportSpec {
    match parse(args)
        .unwrap_or_else(|e| panic!("{args:?}: {e}"))
        .route
    {
        AgentRoute::Export(spec) => spec,
        other => panic!("not an export: {other:?}"),
    }
}

#[test]
fn every_flag_reaches_the_spec() {
    let s = spec(&[
        "ai",
        "export",
        "/v/a.mkv",
        "--format",
        "gif",
        "--size",
        "480p",
        "--quality",
        "small",
        "--look",
        "vivid",
        "--brightness",
        "-0.25",
        "--trim",
        "1.5..4",
        "--out",
        "/tmp/a.GIF",
        "--force",
    ]);
    assert_eq!(s.input, PathBuf::from("/v/a.mkv"));
    assert_eq!(s.format.as_deref(), Some("gif"));
    assert_eq!(s.size.as_deref(), Some("480p"));
    assert_eq!(s.quality.as_deref(), Some("small"));
    assert_eq!(s.look.as_deref(), Some("vivid"));
    assert_eq!(s.brightness_milli, -250);
    assert_eq!(s.trim, TrimRange::parse("1.5..4").unwrap());
    assert_eq!(s.out, Some(PathBuf::from("/tmp/a.GIF")));
    assert!(s.force);
}

#[test]
fn a_bare_export_changes_nothing_and_waits() {
    let call = parse(&["agent", "export", "/v/a.mkv"]).unwrap();
    assert!(call.wait);
    assert_eq!(
        call.route,
        AgentRoute::Export(ExportSpec::new("/v/a.mkv".into()))
    );
    assert!(
        !parse(&["agent", "export", "/v/a.mkv", "--no-wait"])
            .unwrap()
            .wait
    );
}

#[test]
fn values_a_daemon_would_refuse_are_refused_at_the_command_line() {
    for bad in [
        &["agent", "export"][..],
        &["agent", "export", "/v/a.mkv", "--format", "avi"],
        &["agent", "export", "/v/a.mkv", "--size", "4k"],
        &["agent", "export", "/v/a.mkv", "--quality", "best"],
        &["agent", "export", "/v/a.mkv", "--look", "sepia"],
        &["agent", "export", "/v/a.mkv", "--brightness", "0.31"],
        &["agent", "export", "/v/a.mkv", "--brightness", "-0.301"],
        &["agent", "export", "/v/a.mkv", "--brightness", "nan"],
        &["agent", "export", "/v/a.mkv", "--trim", "5"],
        &["agent", "export", "/v/a.mkv", "--trim", "4..2"],
        &["agent", "export", "/v/a.mkv", "--trim", "-1..2"],
        &[
            "agent",
            "export",
            "/v/a.mkv",
            "--out",
            "/tmp/no/such/dir/a.mp4",
        ],
        &["agent", "export", "/v/a.mkv", "--out", "/tmp/a.mkv"],
        &["agent", "export", "/v/a.mkv", "--wait", "--no-wait"],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}
