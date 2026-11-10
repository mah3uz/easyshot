#![allow(clippy::unwrap_used)]
use crate::common;

use common::*;
use es_geom::Rect;
use es_model::platform::{OutputId, OutputInfo, Transform};
use es_record::Clock;
use es_record::gsr::{early_exit, gsr_args};
use es_record::{
    Codec, Container, Error, GsrRecorder, Quality, Recorder, RecorderEvent, RegionUnits,
};
use futures::executor::block_on;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

struct Rig {
    tmp: tempfile::TempDir,
    clock: Arc<FakeClock>,
}

impl Rig {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            clock: Arc::new(FakeClock::new()),
        }
    }

    fn log(&self) -> std::path::PathBuf {
        self.tmp.path().join("log")
    }

    fn recorder(&self, portal: bool, mode: &str) -> GsrRecorder {
        let r = if portal {
            GsrRecorder::portal()
        } else {
            GsrRecorder::native().trust_kms()
        };
        r.with_program(fake("fake-gsr"))
            .with_ipc_dir(self.tmp.path())
            .with_clock(self.clock.clone())
            .with_timing(timing())
            .with_env("FAKE_LOG", &self.log().to_string_lossy())
            .with_env("FAKE_MODE", mode)
    }

    fn lines(&self) -> Vec<String> {
        read_log(&self.log())
    }
}

fn argv(lines: &[String]) -> String {
    lines
        .iter()
        .find_map(|l| l.strip_prefix("argv "))
        .unwrap()
        .to_owned()
}

fn strip_ipc(a: &str, dir: &Path) -> String {
    let marker = "-ipc ";
    let i = a.find(marker).unwrap() + marker.len();
    let rest = &a[i..];
    let end = rest.find(' ').unwrap();
    let sock = &rest[..end];
    assert!(sock.starts_with(&dir.to_string_lossy().into_owned()));
    format!("{}-ipc SOCK{}", &a[..i - marker.len()], &rest[end..])
}

#[test]
fn an_output_recording_passes_the_documented_flags() {
    let rig = Rig::new();
    let s = spec(rig.tmp.path(), output("DP-2"));
    let out = s.dir.join("screen.mkv");
    let h = rig.recorder(false, "").start(s).unwrap();
    block_on(h.stop()).unwrap();
    let a = strip_ipc(&argv(&rig.lines()), rig.tmp.path());
    assert_eq!(
        a,
        format!(
            "-w DP-2 -f 60 -k h264 -c mkv -q very_high -cursor yes -ipc SOCK -write-first-frame-ts yes -o {}",
            out.display()
        )
    );
}

#[test]
fn a_region_uses_global_logical_coordinates_and_system_and_mic_audio_are_two_tracks() {
    let mut s = spec(Path::new("/r"), region("DP-2", 3083.0, 382.0, 800.0, 600.0));
    s.system_audio = true;
    s.mic = Some("alsa_input.usb-rode".into());
    s.cursor = false;
    s.fps = 30;
    s.codec = Codec::H265;
    s.container = Container::Mp4;
    s.quality = Quality::Ultra;
    let a = gsr_args(
        &s,
        false,
        &RegionUnits::Logical,
        Path::new("/run/s.sock"),
        Path::new("/r/screen.mp4"),
    )
    .join(" ");
    assert_eq!(
        a,
        "-w 800x600+3083+382 -f 30 -k hevc -c mp4 -q ultra -a default_output -a device:alsa_input.usb-rode \
         -cursor no -ipc /run/s.sock -write-first-frame-ts yes -o /r/screen.mp4"
    );
}

fn audio_of(system: bool, mic: Option<&str>) -> Vec<String> {
    let mut s = spec(Path::new("/r"), output("DP-1"));
    s.system_audio = system;
    s.mic = mic.map(str::to_owned);
    let a = gsr_args(
        &s,
        false,
        &RegionUnits::Logical,
        Path::new("/s"),
        Path::new("/o"),
    );
    a.iter()
        .enumerate()
        .filter(|(_, x)| *x == "-a")
        .map(|(i, _)| a[i + 1].clone())
        .collect()
}

#[test]
fn every_microphone_setting_becomes_a_name_the_recorder_accepts() {
    for (mic, want) in [
        ("", "default_input"),
        ("default", "default_input"),
        ("default_input", "default_input"),
        ("default_output", "default_output"),
        ("device:x", "device:x"),
        (
            "alsa_input.pci-0000_79_00.6.analog-stereo",
            "device:alsa_input.pci-0000_79_00.6.analog-stereo",
        ),
    ] {
        assert_eq!(audio_of(false, Some(mic)), [want], "mic {mic:?}");
    }
}

#[test]
fn one_audio_source_is_one_track_and_two_are_two_tracks_system_first() {
    assert!(audio_of(false, None).is_empty());
    assert_eq!(audio_of(true, None), ["default_output"]);
    assert_eq!(
        audio_of(true, Some("default")),
        ["default_output", "default_input"]
    );
}

#[test]
fn the_recorders_early_exit_codes_say_what_went_wrong() {
    use es_record::Error;
    assert!(matches!(
        early_exit(Some(50), "x", "list"),
        Error::AudioDeviceMissing
    ));
    assert!(matches!(
        early_exit(Some(51), "x", ""),
        Error::AreaOffMonitor
    ));
    let other = early_exit(Some(1), "exit status: 1", "boom").to_string();
    assert!(
        other.contains("exit status: 1") && other.contains("boom"),
        "{other}"
    );
}

#[test]
fn portal_mode_asks_the_portal_instead_of_naming_a_target() {
    let s = spec(Path::new("/r"), output("DP-2"));
    let a = gsr_args(
        &s,
        true,
        &RegionUnits::Logical,
        Path::new("/s"),
        Path::new("/o"),
    )
    .join(" ");
    assert!(a.starts_with("-w portal -restore-portal-session yes -f 60"));
}

#[test]
fn pause_and_resume_go_over_ipc_and_split_the_recorded_time() {
    let rig = Rig::new();
    let mut h = rig
        .recorder(false, "")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    let events = h.events();
    h.pause().unwrap();
    h.pause().unwrap();
    h.resume().unwrap();
    let out = block_on(h.stop()).unwrap();
    let log = rig.lines();
    assert!(
        log.iter()
            .any(|l| l.contains(r#""name":"set-paused","data":true"#))
    );
    assert!(
        log.iter()
            .any(|l| l.contains(r#""name":"set-paused","data":false"#))
    );
    assert!(!log.contains(&"sigusr2".to_owned()));
    assert_eq!(out.segments.len(), 2);
    assert!(out.segments[0].1 <= out.segments[1].0);
    assert_eq!(out.first_frame_monotonic_us, 123_456);
    let got: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert_eq!(
        got,
        [
            RecorderEvent::Started,
            RecorderEvent::Paused,
            RecorderEvent::Resumed
        ]
    );
}

#[test]
fn without_a_socket_pause_falls_back_to_sigusr2() {
    let rig = Rig::new();
    let mut h = rig
        .recorder(false, "no_ipc")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    h.pause().unwrap();
    std::thread::sleep(Duration::from_millis(150));
    h.resume().unwrap();
    block_on(h.stop()).unwrap();
    let n = rig.lines().iter().filter(|l| *l == "sigusr2").count();
    assert_eq!(n, 2);
}

#[test]
fn a_refused_ipc_pause_also_falls_back_to_sigusr2() {
    let rig = Rig::new();
    let mut h = rig
        .recorder(false, "bad_ipc")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    h.pause().unwrap();
    let _ = block_on(h.stop());
    assert!(rig.lines().contains(&"sigusr2".to_owned()));
}

#[test]
fn stop_goes_over_ipc_and_the_final_path_comes_from_stdout() {
    let rig = Rig::new();
    let s = spec(rig.tmp.path(), output("DP-2"));
    let want = s.dir.join("screen.mkv");
    let h = rig.recorder(false, "").start(s).unwrap();
    let out = block_on(h.stop()).unwrap();
    assert_eq!(out.file, want);
    let log = rig.lines();
    assert!(
        log.iter()
            .any(|l| l.starts_with("ipc") && l.contains(r#""stop""#))
    );
    assert!(!log.contains(&"sigint".to_owned()));
    assert!(process_gone(pid_in(&log)));
}

#[test]
fn the_ipc_socket_lives_in_a_fresh_private_folder() {
    use std::os::unix::fs::PermissionsExt;
    let rig = Rig::new();
    let h = rig
        .recorder(false, "")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    let a = argv(&rig.lines());
    let sock = a.split(' ').skip_while(|w| *w != "-ipc").nth(1).unwrap();
    let folder = Path::new(sock).parent().unwrap().to_path_buf();
    assert_ne!(folder, rig.tmp.path());
    let mode = std::fs::metadata(&folder).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    block_on(h.stop()).unwrap();
    assert!(!folder.exists());
}

#[test]
fn the_stop_ladder_escalates_to_sigint_and_leaves_nothing_running() {
    let rig = Rig::new();
    let h = rig
        .recorder(false, "ignore_ipc_stop")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    let before = rig.clock.now();
    let out = block_on(h.stop()).unwrap();
    let waited = rig.clock.now() - before;
    assert!(waited >= Duration::from_secs(5) && waited < Duration::from_secs(10));
    let log = rig.lines();
    assert!(log.contains(&"sigint".to_owned()));
    assert!(out.file.is_file());
    assert!(process_gone(pid_in(&log)));
}

#[test]
fn a_recorder_that_ignores_sigint_is_killed_at_ten_seconds() {
    let rig = Rig::new();
    let h = rig
        .recorder(false, "ignore_ipc_stop,ignore_int")
        .start(spec(rig.tmp.path(), output("DP-2")))
        .unwrap();
    let before = rig.clock.now();
    let r = block_on(h.stop());
    assert!(rig.clock.now() - before >= Duration::from_secs(10));
    assert!(matches!(r, Err(Error::NotFound(_))));
    let log = rig.lines();
    assert!(log.contains(&"sigint".to_owned()));
    assert!(process_gone(pid_in(&log)));
}

#[test]
fn the_portal_watchdog_gives_up_after_ten_seconds() {
    let rig = Rig::new();
    let r = rig
        .recorder(true, "hang_portal")
        .start(spec(rig.tmp.path(), output("DP-2")));
    assert!(matches!(r, Err(Error::Timeout)));
    let t = rig.clock.now();
    assert!(
        t >= Duration::from_secs(10) && t < Duration::from_secs(12),
        "{t:?}"
    );
    assert!(process_gone(pid_in(&rig.lines())));
}

#[test]
fn discard_stops_the_recorder_and_deletes_the_folder() {
    let rig = Rig::new();
    let s = spec(rig.tmp.path(), output("DP-2"));
    let dir = s.dir.clone();
    let h = rig.recorder(false, "").start(s).unwrap();
    block_on(h.discard()).unwrap();
    assert!(!dir.exists());
    assert!(process_gone(pid_in(&rig.lines())));
}

#[test]
fn probe_reports_what_is_missing() {
    let rig = Rig::new();
    let p = GsrRecorder::native()
        .with_program("/nonexistent/gpu-screen-recorder")
        .probe();
    assert!(!p.available);
    let p = GsrRecorder::native()
        .with_program(fake("fake-gsr"))
        .with_kms_server(fake("fake-wf"))
        .probe();
    assert_eq!(
        p.reason.as_deref(),
        Some("gsr-kms-server lacks cap_sys_admin")
    );
    assert!(rig.recorder(false, "").probe().available);
    assert!(rig.recorder(true, "").probe().available);
}

#[test]
fn a_recorder_that_dies_on_start_reports_its_stderr() {
    let rig = Rig::new();
    let r = GsrRecorder::native()
        .with_program("/bin/false")
        .with_clock(rig.clock.clone())
        .start(spec(rig.tmp.path(), output("DP-2")));
    assert!(matches!(r, Err(Error::Protocol(_))));
}

#[test]
fn a_region_recording_on_a_scaled_output_is_sent_in_physical_pixels() {
    let rig = Rig::new();
    let outputs = [OutputInfo {
        id: OutputId("eDP-1".into()),
        description: String::new(),
        logical: Rect::new(0.0, 0.0, 1280.0, 800.0),
        physical: (2560, 1600),
        scale: 2.0,
        refresh_mhz: 60_000,
        transform: Transform::Normal,
        focused: None,
    }];
    let r = rig.recorder(false, "");
    let s = spec(rig.tmp.path(), region("eDP-1", 100.0, 50.0, 300.0, 200.0));
    r.prepare(&s, &outputs);
    block_on(r.start(s).unwrap().stop()).unwrap();
    assert!(argv(&rig.lines()).starts_with("-w 600x400+200+100 "));
}

#[test]
fn an_area_on_an_offset_output_is_probed_once_and_sent_on_the_recorders_monitor() {
    let rig = Rig::new();
    let outputs = [OutputInfo {
        id: OutputId("DP-2".into()),
        description: String::new(),
        logical: Rect::new(2883.0, 182.0, 5443.0, 1622.0),
        physical: (2560, 1440),
        scale: 1.0,
        refresh_mhz: 60_000,
        transform: Transform::Normal,
        focused: None,
    }];
    let r = rig
        .recorder(false, "")
        .with_env("FAKE_MONITORS", "DP-2=2560x1440+0+0");
    for _ in 0..2 {
        let s = spec(rig.tmp.path(), region("DP-2", 3000.0, 400.0, 800.0, 600.0));
        r.prepare(&s, &outputs);
        block_on(r.start(s).unwrap().stop()).unwrap();
    }
    let lines = rig.lines();
    let probes: Vec<_> = lines.iter().filter(|l| l.starts_with("probe ")).collect();
    assert_eq!(probes.len(), 1, "one probe per output layout: {lines:?}");
    assert!(
        probes[0].contains("-c mkv"),
        "without a container the recorder never lists its monitors"
    );
    assert!(argv(&lines).starts_with("-w 800x600+117+218 "));
}
