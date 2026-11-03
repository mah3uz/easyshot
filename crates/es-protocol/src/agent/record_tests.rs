use super::*;
use crate::agent::{AgentReply, AgentRoute, ErrorCode, Geometry, ReplyKind, WindowSelector};
use crate::{Error, Route};
use serde_json::json;

fn parsed(route: RecordRoute) -> Result<Route, Error> {
    let route = Route::Agent(AgentRoute::Record(route));
    Route::parse(&route.as_str(), &route.args())
}

fn start() -> RecordSpec {
    RecordSpec::new(AgentTarget::Fullscreen)
}

#[test]
fn every_option_survives_the_route_arguments() {
    let spec = RecordSpec {
        target: AgentTarget::Geometry(Geometry::parse("10,20 640x480").unwrap()),
        audio: Some(Audio::Both),
        fps: Some(60),
        keystrokes: true,
        max_seconds: MAX_SECONDS,
    };
    for each in [
        RecordRoute::Start(spec),
        RecordRoute::Stop(Some("rec-1a2b".into())),
        RecordRoute::Discard(None),
        RecordRoute::Status,
    ] {
        let expected = Route::Agent(AgentRoute::Record(each.clone()));
        assert_eq!(parsed(each).unwrap(), expected);
    }
}

#[test]
fn a_start_without_a_target_or_with_two_is_refused() {
    let args = |extra: &[(&str, ArgValue)]| {
        let mut args = AgentRoute::Record(RecordRoute::Start(start())).args();
        args.0.remove("fullscreen");
        for (k, v) in extra {
            args.0.insert((*k).to_owned(), v.clone());
        }
        args
    };
    assert!(Route::parse("agent/record-start", &args(&[])).is_err());
    let two = args(&[
        ("output", ArgValue::Str("DP-1".into())),
        ("window", ArgValue::Str("foot".into())),
    ]);
    assert!(Route::parse("agent/record-start", &two).is_err());
}

#[test]
fn out_of_range_and_unknown_options_are_refused() {
    let with = |key: &str, value: ArgValue| {
        let mut args = AgentRoute::Record(RecordRoute::Start(start())).args();
        args.0.insert(key.to_owned(), value);
        Route::parse("agent/record-start", &args)
    };
    assert!(with("fps", ArgValue::Int(60)).is_ok());
    for bad in [0, 45, 120, -30] {
        assert!(with("fps", ArgValue::Int(bad)).is_err(), "fps {bad}");
    }
    assert!(with("max_seconds", ArgValue::Int(600)).is_ok());
    for bad in [0, 601, -1] {
        assert!(with("max_seconds", ArgValue::Int(bad)).is_err(), "{bad}");
    }
    assert!(with("audio", ArgValue::Str("loud".into())).is_err());
    assert!(with("shell", ArgValue::Str("rm".into())).is_err());
    let mut stop = AgentRoute::Record(RecordRoute::Stop(None)).args();
    stop.0.insert("fullscreen".into(), ArgValue::Bool(true));
    assert!(Route::parse("agent/record-stop", &stop).is_err());
}

#[test]
fn an_id_is_plain_letters_digits_and_dashes() {
    assert!(parsed(RecordRoute::Stop(Some("rec-0f3a".into()))).is_ok());
    for bad in ["", "a b", "../x", "a;b", &"a".repeat(65)] {
        assert!(parsed(RecordRoute::Discard(Some(bad.into()))).is_err());
    }
}

#[test]
fn only_a_start_names_a_target_for_the_access_scope() {
    let window = AgentTarget::Window(WindowSelector::any("foot").unwrap());
    let start = AgentRoute::Record(RecordRoute::Start(RecordSpec::new(window.clone())));
    assert_eq!(start.target(), Some(&window));
    for other in [
        RecordRoute::Stop(None),
        RecordRoute::Status,
        RecordRoute::Discard(None),
    ] {
        assert_eq!(AgentRoute::Record(other).target(), None);
    }
}

#[test]
fn recording_replies_are_flat_and_read_back() {
    let stopped = RecordingInfo {
        id: Some("rec-1".into()),
        path: Some("/v/a.mkv".into()),
        duration_ms: Some(2500),
        width: Some(1280),
        height: Some(720),
        bytes: Some(4096),
        ..RecordingInfo::default()
    };
    let started = RecordingInfo {
        id: Some("rec-1".into()),
        state: Some("recording".into()),
        target: Some("output DP-1".into()),
        max_seconds: Some(120),
        ..RecordingInfo::default()
    };
    let idle = RecordingInfo {
        state: Some("idle".into()),
        ..RecordingInfo::default()
    };
    for (info, expected) in [
        (
            stopped,
            json!({"ok": true, "kind": "recording", "id": "rec-1", "path": "/v/a.mkv",
                   "duration_ms": 2500, "width": 1280, "height": 720, "bytes": 4096}),
        ),
        (
            started,
            json!({"ok": true, "kind": "recording", "id": "rec-1", "state": "recording",
                   "target": "output DP-1", "max_seconds": 120}),
        ),
        (
            idle,
            json!({"ok": true, "kind": "recording", "state": "idle"}),
        ),
    ] {
        let reply = AgentReply {
            recording: Some(Box::new(info)),
            ..AgentReply::ok(ReplyKind::Recording)
        };
        let text = reply.to_json();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            expected
        );
        assert_eq!(AgentReply::from_json(&text).unwrap(), reply);
    }
    let refused = AgentReply::failed(ReplyKind::Recording, ErrorCode::InvalidArguments, "busy");
    assert_eq!(AgentReply::from_json(&refused.to_json()).unwrap(), refused);
}
