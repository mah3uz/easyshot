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

const ERROR_TEXT: &str = "gsr error: window 1x1+0+0 doesn't match any monitor. Available: \
    DP-2 (2560x1440+2883+182), eDP-1 (2560x1600+0+0)";

#[test]
fn the_monitor_list_is_parsed_from_the_error_text() {
    let m = parse_monitor_list(ERROR_TEXT);
    assert_eq!(m.len(), 2);
    assert_eq!(
        (m[0].name.as_str(), m[0].width, m[0].height, m[0].x, m[0].y),
        ("DP-2", 2560, 1440, 2883, 182)
    );
    assert_eq!((m[1].name.as_str(), m[1].width), ("eDP-1", 2560));
}

#[test]
fn a_printed_size_equal_to_the_logical_size_means_logical_coordinates() {
    let outs = [
        output("DP-2", (2883.0, 182.0, 2560.0, 1440.0), (2560, 1440), 1.0),
        output("eDP-1", (0.0, 0.0, 1280.0, 800.0), (2560, 1600), 2.0),
    ];
    let printed = parse_monitor_list("DP-2 (2560x1440+2883+182), eDP-1 (1280x800+0+0)");
    assert_eq!(decide_units(&printed, &outs), RegionUnits::Logical);
}

#[test]
fn a_printed_physical_size_on_a_scaled_output_maps_regions_to_physical_pixels() {
    let outs = [
        output("DP-2", (2883.0, 182.0, 2560.0, 1440.0), (2560, 1440), 1.0),
        output("eDP-1", (0.0, 0.0, 1280.0, 800.0), (2560, 1600), 2.0),
    ];
    let printed = parse_monitor_list(ERROR_TEXT);
    let units = decide_units(&printed, &outs);
    let RegionUnits::Physical(maps) = &units else {
        panic!("expected physical");
    };
    assert_eq!(maps.len(), 1);
    let edp = OutputId("eDP-1".into());
    let region = Rect::new(100.0, 50.0, 400.0, 250.0);
    assert_eq!(units.region_target(&edp, region), "600x400+200+100");
    let dp = OutputId("DP-2".into());
    let region = Rect::new(3083.0, 382.0, 3883.0, 982.0);
    assert_eq!(units.region_target(&dp, region), "800x600+3083+382");
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
