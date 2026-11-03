use super::*;

fn round_trip(spec: &ExportSpec) -> Result<ExportSpec, String> {
    let mut args = RouteArgs::default();
    spec.write_args(&mut args);
    ExportSpec::from_args(&args)
}

fn with(key: &str, value: ArgValue) -> Result<ExportSpec, String> {
    let mut args = RouteArgs::default();
    ExportSpec::new("/v/a.mkv".into()).write_args(&mut args);
    args.0.insert(key.to_owned(), value);
    ExportSpec::from_args(&args)
}

fn text(s: &str) -> ArgValue {
    ArgValue::Str(s.to_owned())
}

#[test]
fn trim_ranges_take_decimals_and_either_end() {
    let r = |a, b| TrimRange {
        start_ms: a,
        end_ms: b,
    };
    assert_eq!(TrimRange::parse("1.5..4"), Ok(r(Some(1500), Some(4000))));
    assert_eq!(TrimRange::parse("..10"), Ok(r(None, Some(10_000))));
    assert_eq!(TrimRange::parse("2.25.."), Ok(r(Some(2250), None)));
    assert_eq!(TrimRange::parse("0..0.001"), Ok(r(Some(0), Some(1))));
}

#[test]
fn bad_trim_ranges_are_refused() {
    for bad in [
        "",
        "..",
        "5",
        "1-2",
        "1...2",
        "-1..2",
        "1..-2",
        "a..2",
        "1e3..2",
        ".5..1",
        "3..3",
        "4..2",
        "1.2.3..5",
        " 1..2",
        "inf..2",
        "0..360001",
    ] {
        assert!(TrimRange::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn every_option_survives_the_route_arguments() {
    let spec = ExportSpec {
        input: "/v/a.mkv".into(),
        format: Some("webm".into()),
        size: Some("720p".into()),
        quality: Some("small".into()),
        look: Some("mono".into()),
        brightness_milli: -250,
        trim: TrimRange::parse("1.5..4").unwrap(),
        out: Some("/o/b.webm".into()),
        force: true,
    };
    assert_eq!(round_trip(&spec).unwrap(), spec);
    let plain = ExportSpec::new("/v/a.mkv".into());
    assert_eq!(round_trip(&plain).unwrap(), plain);
}

#[test]
fn the_daemon_refuses_values_a_command_would_not_send() {
    assert!(with(INPUT_ARG, text("a.mkv")).is_err());
    assert!(with(INPUT_ARG, text("")).is_err());
    assert!(with(INPUT_ARG, text("/v/a\0.mkv")).is_err());
    assert!(with(INPUT_ARG, ArgValue::Int(1)).is_err());
    for (key, bad) in [
        (FORMAT_ARG, "avi"),
        (SIZE_ARG, "4k"),
        (QUALITY_ARG, "best"),
        (LOOK_ARG, "sepia"),
    ] {
        assert!(with(key, text(bad)).is_err(), "{key}");
    }
    assert!(with(BRIGHTNESS_ARG, ArgValue::Int(301)).is_err());
    assert!(with(BRIGHTNESS_ARG, ArgValue::Int(-301)).is_err());
    assert!(with(BRIGHTNESS_ARG, ArgValue::Int(-300)).is_ok());
    assert!(with(TRIM_START_ARG, ArgValue::Int(-1)).is_err());
    assert!(with(TRIM_END_ARG, text("5")).is_err());
    assert!(with(FORCE_ARG, text("true")).is_err());
}

#[test]
fn an_out_file_must_match_the_format() {
    assert!(with(OUT_ARG, text("/o/b.mp4")).is_ok());
    assert!(with(OUT_ARG, text("/o/b.MKV")).is_err());
    assert!(with(OUT_ARG, text("/o/b")).is_err());
    let mut args = RouteArgs::default();
    ExportSpec::new("/v/a.mkv".into()).write_args(&mut args);
    args.0.insert(FORMAT_ARG.into(), text("gif"));
    args.0.insert(OUT_ARG.into(), text("/o/b.mp4"));
    assert!(ExportSpec::from_args(&args).is_err());
    args.0.insert(OUT_ARG.into(), text("/o/b.GIF"));
    assert!(ExportSpec::from_args(&args).is_ok());
}

#[test]
fn a_trim_that_ends_before_it_starts_is_refused() {
    let mut args = RouteArgs::default();
    ExportSpec::new("/v/a.mkv".into()).write_args(&mut args);
    args.0.insert(TRIM_START_ARG.into(), ArgValue::Int(5000));
    args.0.insert(TRIM_END_ARG.into(), ArgValue::Int(5000));
    assert!(ExportSpec::from_args(&args).is_err());
}
