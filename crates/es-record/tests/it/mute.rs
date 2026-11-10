#![allow(clippy::unwrap_used)]
use crate::common::*;

use es_record::mute::{Mutes, parse_streams, sources, targets};
use es_record::project::CommandRunner;
use es_record::{AudioSource, Error, GsrRecorder, Recorder};
use futures::executor::block_on;
use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex};

fn ok(stdout: String) -> std::io::Result<Output> {
    Ok(Output {
        status: std::process::ExitStatus::from_raw(0),
        stdout: stdout.into_bytes(),
        stderr: Vec::new(),
    })
}

fn listing(pid: u32, ids: &[u32]) -> String {
    let mut rows: Vec<String> = ids
        .iter()
        .map(|id| {
            format!(
                r#"{{"index":{id},"source":55,"properties":{{"application.process.id":"{pid}","application.name":"gpu-screen-recorder"}}}}"#
            )
        })
        .collect();
    rows.push(r#"{"index":3,"source":55,"properties":{"application.process.id":"1","application.name":"obs"}}"#.into());
    format!("[{}]", rows.join(","))
}

/// A sound server that shows one stream per `-a` track the recorder was started with.
struct Pactl {
    log: PathBuf,
    calls: Mutex<Vec<String>>,
    after_stop: Mutex<Vec<String>>,
}

impl Pactl {
    fn new(log: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            log,
            calls: Mutex::default(),
            after_stop: Mutex::default(),
        })
    }

    fn mutes(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.starts_with("set-source-output-mute"))
            .cloned()
            .collect()
    }
}

impl CommandRunner for Pactl {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        assert_eq!(program, "pactl");
        self.calls.lock().unwrap().push(args.join(" "));
        if args[0] == "set-source-output-mute"
            && read_log(&self.log).iter().any(|l| l.starts_with("finish"))
        {
            self.after_stop.lock().unwrap().push(args.join(" "));
        }
        if args[0] != "-f" {
            return ok(String::new());
        }
        let log = read_log(&self.log);
        let Some(pid) = log.iter().rev().find_map(|l| l.strip_prefix("pid ")) else {
            return ok("[]".into());
        };
        let argv = log
            .iter()
            .rev()
            .find_map(|l| l.strip_prefix("argv "))
            .unwrap();
        let tracks: Vec<&str> = argv
            .split(" -a ")
            .skip(1)
            .map(|t| t.split(' ').next().unwrap())
            .collect();
        let ids: Vec<u32> = (0..tracks.len() as u32).map(|i| 70 + i).collect();
        ok(listing(pid.parse().unwrap(), &ids))
    }
}

#[test]
fn only_the_recorders_own_streams_are_found_oldest_first() {
    let json = listing(42, &[81, 80]);
    assert_eq!(parse_streams(&json, 42).unwrap(), [80, 81]);
    assert!(parse_streams(&json, 7).unwrap().is_empty());
    assert!(matches!(
        parse_streams("not json", 42),
        Err(Error::Parse { .. })
    ));
}

#[test]
fn each_source_maps_to_its_own_stream_in_track_order() {
    let both = sources(true, true);
    assert_eq!(both, [AudioSource::System, AudioSource::Mic]);
    assert_eq!(targets(&[80, 81], &both, AudioSource::Mic).unwrap(), [81]);
    assert_eq!(
        targets(&[80, 81], &both, AudioSource::System).unwrap(),
        [80]
    );
    assert_eq!(
        targets(&[90], &sources(false, true), AudioSource::Mic).unwrap(),
        [90]
    );
    assert!(matches!(
        targets(&[80], &both, AudioSource::Mic),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        targets(&[], &both, AudioSource::Mic),
        Err(Error::NotFound(_))
    ));
}

fn rig(tmp: &Path) -> (GsrRecorder, Arc<Pactl>) {
    let log = tmp.join("log");
    let pactl = Pactl::new(log.clone());
    let recorder = GsrRecorder::native()
        .trust_kms()
        .with_program(fake("fake-gsr"))
        .with_ipc_dir(tmp)
        .with_clock(Arc::new(FakeClock::new()))
        .with_timing(timing())
        .with_env("FAKE_LOG", &log.to_string_lossy())
        .with_audio_runner(pactl.clone());
    (recorder, pactl)
}

fn both(tmp: &Path) -> es_record::RecordSpec {
    let mut s = spec(tmp, output("DP-2"));
    s.system_audio = true;
    s.mic = Some("default".into());
    s
}

fn tracks(log: &[String]) -> Vec<String> {
    log.iter()
        .filter_map(|l| l.strip_prefix("argv "))
        .map(|a| {
            a.split(" -a ")
                .skip(1)
                .map(|t| t.split(' ').next().unwrap())
                .collect::<Vec<_>>()
                .join(" + ")
        })
        .collect()
}

#[test]
fn both_sounds_are_two_tracks_system_first_and_each_mutes_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, pactl) = rig(tmp.path());
    let mut h = recorder.start(both(tmp.path())).unwrap();
    h.set_muted(AudioSource::Mic, true).unwrap();
    h.set_muted(AudioSource::System, true).unwrap();
    h.set_muted(AudioSource::Mic, false).unwrap();
    block_on(h.stop()).unwrap();
    assert_eq!(
        tracks(&read_log(&tmp.path().join("log"))),
        ["default_output + default_input"],
        "one launch, two tracks, no mixed launch"
    );
    assert_eq!(
        pactl.mutes(),
        [
            "set-source-output-mute 70 0",
            "set-source-output-mute 71 0",
            "set-source-output-mute 71 1",
            "set-source-output-mute 70 1",
            "set-source-output-mute 71 0",
            "set-source-output-mute 70 0",
            "set-source-output-mute 71 0",
        ]
    );
}

#[test]
fn stopping_a_two_track_recording_leaves_the_file_as_the_recorder_wrote_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, _) = rig(tmp.path());
    let h = recorder.start(both(tmp.path())).unwrap();
    let out = block_on(h.stop()).unwrap();
    assert!(out.file.is_file());
    assert!(!out.file.with_extension("merged.mkv").exists());
}

#[test]
fn without_a_sound_server_runner_nothing_is_muted() {
    let tmp = tempfile::tempdir().unwrap();
    let _ = rig(tmp.path());
    let recorder = GsrRecorder::native()
        .trust_kms()
        .with_program(fake("fake-gsr"))
        .with_ipc_dir(tmp.path())
        .with_clock(Arc::new(FakeClock::new()))
        .with_timing(timing())
        .with_env("FAKE_LOG", &tmp.path().join("log").to_string_lossy());
    let mut h = recorder.start(both(tmp.path())).unwrap();
    assert!(matches!(
        h.set_muted(AudioSource::Mic, true),
        Err(Error::Unsupported)
    ));
    block_on(h.stop()).unwrap();
}

#[test]
fn a_wrong_stream_count_mutes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let pactl = Pactl::new(tmp.path().join("missing-log"));
    let mutes = Mutes::new(pactl.clone(), sources(true, true));
    assert!(matches!(
        mutes.set(42, AudioSource::Mic, true),
        Err(Error::NotFound(_))
    ));
    assert!(pactl.mutes().is_empty());
}

#[test]
fn a_recording_never_starts_muted_by_what_an_earlier_recording_left_in_the_sound_server() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, pactl) = rig(tmp.path());
    let h = recorder.start(both(tmp.path())).unwrap();
    assert_eq!(
        pactl.mutes(),
        ["set-source-output-mute 70 0", "set-source-output-mute 71 0"]
    );
    block_on(h.stop()).unwrap();
}

#[test]
fn muting_the_mic_during_a_recording_is_undone_before_the_recorder_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, pactl) = rig(tmp.path());
    let mut h = recorder.start(both(tmp.path())).unwrap();
    h.set_muted(AudioSource::Mic, true).unwrap();
    block_on(h.stop()).unwrap();
    let mutes = pactl.mutes();
    assert_eq!(
        mutes[mutes.len() - 2..],
        ["set-source-output-mute 70 0", "set-source-output-mute 71 0"]
    );
    assert!(
        pactl.after_stop.lock().unwrap().is_empty(),
        "the stored state must be unmuted before the recorder goes away"
    );
}
