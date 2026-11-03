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

fn spec(args: &[&str]) -> OcrSpec {
    match parse(args)
        .unwrap_or_else(|e| panic!("{args:?}: {e}"))
        .route
    {
        AgentRoute::Ocr(spec) => spec,
        other => panic!("not an ocr: {other:?}"),
    }
}

#[test]
fn each_source_flag_builds_its_typed_input() {
    use AgentTarget::*;
    let target = |args: &[&str]| spec(args).input;
    assert_eq!(
        target(&["agent", "ocr", "--fullscreen"]),
        OcrInput::Target(Fullscreen)
    );
    assert_eq!(
        target(&["ai", "ocr", "--geometry", "-10,20 300x400"]),
        OcrInput::Target(Geometry(
            es_protocol::agent::Geometry::parse("-10,20 300x400").unwrap()
        ))
    );
    assert_eq!(
        target(&["ai", "ocr", "--window", "foot"]),
        OcrInput::Target(Window(WindowSelector::any("foot").unwrap()))
    );
    assert_eq!(
        target(&["ai", "ocr", "--output", "DP-1"]),
        OcrInput::Target(Output(OutputName::new("DP-1").unwrap()))
    );
    assert_eq!(
        target(&["ai", "ocr", "--file", "/tmp/a.png"]),
        OcrInput::File("/tmp/a.png".into())
    );
}

#[test]
fn a_relative_file_is_made_absolute_and_only_images_are_read() {
    let OcrInput::File(path) = spec(&["agent", "ocr", "--file", "shot.JPEG"]).input else {
        panic!("not a file");
    };
    assert!(path.is_absolute() && path.ends_with("shot.JPEG"));
    assert!(parse(&["agent", "ocr", "--file", "notes.txt"]).is_err());
}

#[test]
fn exactly_one_source_is_needed() {
    for bad in [
        &["agent", "ocr"][..],
        &["agent", "ocr", "--fullscreen", "--file", "/tmp/a.png"],
        &["agent", "ocr", "--output", "DP-1", "--window", "foot"],
    ] {
        assert!(parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn languages_default_to_the_setting_and_are_checked() {
    assert!(spec(&["agent", "ocr", "--fullscreen"]).languages.is_empty());
    assert_eq!(
        spec(&["agent", "ocr", "--fullscreen", "--lang", "eng+deu"]).languages,
        ["eng", "deu"]
    );
    assert!(parse(&["agent", "ocr", "--fullscreen", "--lang", "../x"]).is_err());
}

#[test]
fn the_command_waits_for_its_answer() {
    assert!(parse(&["agent", "ocr", "--fullscreen"]).unwrap().wait);
}
