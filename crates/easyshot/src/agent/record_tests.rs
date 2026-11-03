use super::*;
use crate::cli::{Cli, Request};
use clap::Parser;
use es_protocol::AgentRoute;

fn parse(args: &[&str]) -> Result<RecordRoute, clap::Error> {
    let cli = Cli::try_parse_from(
        ["easyshot", "agent", "record"]
            .into_iter()
            .chain(args.iter().copied()),
    )?;
    match cli.request().unwrap() {
        Request::Agent(call) => {
            assert!(call.wait, "a record command waits for its answer");
            match call.route {
                AgentRoute::Record(route) => Ok(route),
                other => panic!("{other:?}"),
            }
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn start_takes_one_target_and_its_options() {
    let RecordRoute::Start(spec) = parse(&[
        "start",
        "--output",
        "DP-1",
        "--audio",
        "mic",
        "--fps",
        "60",
        "--keystrokes",
        "--max-seconds",
        "30",
    ])
    .unwrap() else {
        panic!("not a start");
    };
    assert_eq!(
        spec,
        RecordSpec {
            target: AgentTarget::Output(OutputName::new("DP-1").unwrap()),
            audio: Some(Audio::Mic),
            fps: Some(60),
            keystrokes: true,
            max_seconds: 30,
        }
    );
    let RecordRoute::Start(plain) = parse(&["start", "--fullscreen"]).unwrap() else {
        panic!("not a start");
    };
    assert_eq!(
        plain,
        RecordSpec::new(AgentTarget::Fullscreen),
        "the settings and the default limit apply"
    );
}

#[test]
fn start_refuses_no_target_two_targets_and_out_of_range_options() {
    for bad in [
        &["start"][..],
        &["start", "--output", "DP-1", "--fullscreen"],
        &["start", "--fullscreen", "--fps", "45"],
        &["start", "--fullscreen", "--max-seconds", "601"],
        &["start", "--fullscreen", "--max-seconds", "0"],
        &["start", "--fullscreen", "--audio", "loud"],
        &["stop", "--id", "a b"],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn stop_status_and_discard_name_a_recording_only_when_asked() {
    assert_eq!(parse(&["stop"]).unwrap(), RecordRoute::Stop(None));
    assert_eq!(
        parse(&["stop", "--id", "rec-1"]).unwrap(),
        RecordRoute::Stop(Some("rec-1".into()))
    );
    assert_eq!(parse(&["status"]).unwrap(), RecordRoute::Status);
    assert_eq!(parse(&["discard"]).unwrap(), RecordRoute::Discard(None));
}
