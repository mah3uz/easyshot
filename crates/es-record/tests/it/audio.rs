#![allow(clippy::unwrap_used)]
use es_geom::Rect;
use es_model::platform::{OutputId, OutputInfo, Transform};
use es_record::audio::{Runner, SystemRunner};
use es_record::{
    AudioKind, Error, RegionUnits, decide_units, list_audio_devices, parse_monitor_list,
    parse_pactl_sources,
};

const SOURCES: &str = r#"[
 {"name":"alsa_output.pci-0000_79_00.6.analog-stereo.monitor","description":"Monitor of Ryzen HD Audio",
  "properties":{"device.class":"monitor","media.class":"Audio/Sink"}},
 {"name":"alsa_input.usb-RODE","description":"RODE VideoMic GO II",
  "properties":{"device.class":"sound","media.class":"Audio/Source","device.bus":"usb"}},
 {"name":"easyeffects_source","description":"Easy Effects Source","properties":{"media.class":"Audio/Source/Virtual"}}
]"#;

#[test]
fn sources_are_classified_by_device_class_and_defaults_come_first() {
    let d = parse_pactl_sources(SOURCES).unwrap();
    let names: Vec<_> = d.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(&names[..2], ["default_output", "default_input"]);
    assert_eq!(d[2].kind, AudioKind::SystemOutput);
    assert_eq!(d[3].kind, AudioKind::Microphone);
    assert_eq!(d[3].description, "RODE VideoMic GO II");
    assert_eq!(d[4].kind, AudioKind::Virtual);
}

#[test]
fn malformed_json_is_a_parse_error() {
    assert!(matches!(
        parse_pactl_sources("[{"),
        Err(Error::Parse { .. })
    ));
}

struct Canned(Option<&'static str>);

impl Runner for Canned {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<String> {
        assert_eq!(
            (program, args),
            ("pactl", &["-f", "json", "list", "sources"][..])
        );
        self.0
            .map(str::to_owned)
            .ok_or_else(|| std::io::Error::other("no pactl"))
    }
}

#[test]
fn listing_runs_pactl_with_json_and_reports_a_missing_tool_as_unsupported() {
    assert_eq!(list_audio_devices(&Canned(Some(SOURCES))).unwrap().len(), 5);
    assert!(matches!(
        list_audio_devices(&Canned(None)),
        Err(Error::Unsupported)
    ));
    let _ = SystemRunner;
}

fn output(
    name: &str,
    logical: (f64, f64, f64, f64),
    physical: (u32, u32),
    scale: f64,
) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical: Rect::new(
            logical.0,
            logical.1,
            logical.0 + logical.2,
            logical.1 + logical.3,
        ),
        physical,
        scale,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    }
}

// Printed by gpu-screen-recorder 6.1.6 in headless sway with two outputs, one at scale 1.5.
const ERROR_TEXT: &str = "gsr error: the region 1x1+1000000+1000000 doesn't match any monitor. \
    Available monitors and their regions:\n  \"HEADLESS-1\"    (1706x960+2883+182)\n  \"HEADLESS-2\"    (1920x1080+0+0)\n";

#[test]
fn the_monitor_list_is_parsed_from_the_recorders_error_text() {
    let m = parse_monitor_list(ERROR_TEXT);
    assert_eq!(m.len(), 2);
    assert_eq!(
        (m[0].name.as_str(), m[0].width, m[0].height, m[0].x, m[0].y),
        ("HEADLESS-1", 1706, 960, 2883, 182)
    );
    assert_eq!((m[1].name.as_str(), m[1].width), ("HEADLESS-2", 1920));
}

fn mapped(printed: &str, outs: &[OutputInfo]) -> RegionUnits {
    let units = decide_units(&parse_monitor_list(printed), outs);
    assert!(matches!(units, RegionUnits::Mapped(_)), "{units:?}");
    units
}

fn target(units: &RegionUnits, output: &str, x: f64, y: f64, w: f64, h: f64) -> String {
    units.region_target(&OutputId(output.into()), Rect::new(x, y, x + w, y + h))
}

#[test]
fn a_global_area_on_an_offset_output_lands_on_that_monitor_in_the_recorders_coordinates() {
    let outs = [output(
        "DP-2",
        (2883.0, 182.0, 2560.0, 1440.0),
        (2560, 1440),
        1.0,
    )];
    let global = mapped(r#"  "DP-2"    (2560x1440+2883+182)"#, &outs);
    assert_eq!(
        target(&global, "DP-2", 3000.0, 400.0, 800.0, 600.0),
        "800x600+3000+400"
    );
    let from_zero = mapped(r#"  "DP-2"    (2560x1440+0+0)"#, &outs);
    assert_eq!(
        target(&from_zero, "DP-2", 3000.0, 400.0, 800.0, 600.0),
        "800x600+117+218"
    );
}

#[test]
fn a_scaled_offset_output_maps_by_the_size_the_recorder_printed() {
    let outs = [output(
        "DP-2",
        (2883.0, 182.0, 2560.0 / 1.5, 960.0),
        (2560, 1440),
        1.5,
    )];
    let logical = mapped(r#"  "DP-2"    (1706x960+2883+182)"#, &outs);
    assert_eq!(
        target(&logical, "DP-2", 3000.0, 400.0, 800.0, 600.0),
        "800x600+3000+400"
    );
    assert_eq!(
        target(&logical, "DP-2", 2883.0, 182.0, 2560.0 / 1.5, 960.0),
        "1706x960+2883+182"
    );
    let physical = mapped(r#"  "DP-2"    (2560x1440+0+0)"#, &outs);
    assert_eq!(
        target(&physical, "DP-2", 3001.0, 400.0, 800.0, 600.0),
        "1200x900+177+327"
    );
}

#[test]
fn outputs_side_by_side_each_map_to_their_own_monitor() {
    let outs = [
        output("HEADLESS-2", (0.0, 0.0, 1920.0, 1080.0), (1920, 1080), 1.0),
        output(
            "HEADLESS-1",
            (2883.0, 182.0, 2560.0 / 1.5, 960.0),
            (2560, 1440),
            1.5,
        ),
    ];
    let units = mapped(ERROR_TEXT, &outs);
    assert_eq!(
        target(&units, "HEADLESS-2", 100.0, 50.0, 300.0, 200.0),
        "300x200+100+50"
    );
    assert_eq!(
        target(&units, "HEADLESS-1", 3000.0, 400.0, 800.0, 600.0),
        "800x600+3000+400"
    );
}

#[test]
fn without_a_matching_monitor_the_area_goes_as_global_logical_coordinates() {
    let outs = [output(
        "DP-2",
        (2883.0, 182.0, 2560.0, 1440.0),
        (2560, 1440),
        1.0,
    )];
    let units = decide_units(&parse_monitor_list(ERROR_TEXT), &outs);
    assert_eq!(units, RegionUnits::Logical);
    assert_eq!(
        target(&units, "DP-2", 3000.0, 400.0, 800.0, 600.0),
        "800x600+3000+400"
    );
}

struct Sink(Option<&'static str>);

impl Runner for Sink {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<String> {
        assert_eq!((program, args), ("pactl", ["get-default-sink"].as_slice()));
        self.0
            .map(str::to_owned)
            .ok_or_else(|| std::io::Error::other("no sound server"))
    }
}

#[test]
fn system_sound_is_taken_from_the_default_sinks_monitor() {
    let got = es_record::audio::default_sink_monitor(&Sink(Some("alsa_output.pci.analog\n")));
    assert_eq!(got.as_deref(), Some("alsa_output.pci.analog.monitor"));
    assert_eq!(
        es_record::audio::default_sink_monitor(&Sink(Some("\n"))),
        None
    );
    assert_eq!(es_record::audio::default_sink_monitor(&Sink(None)), None);
}
