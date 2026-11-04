#![allow(clippy::unwrap_used)]
use crate::common::*;

use es_record::mute::{Mutes, Shape, merge_args, merge_tracks, parse_streams, sources, targets};
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

/// A sound server that shows the recorder's streams, either one per source or one per `-a` track.
struct Pactl {
    log: PathBuf,
    shape: Shape,
    calls: Mutex<Vec<String>>,
}

impl Pactl {
    fn new(log: PathBuf, shape: Shape) -> Arc<Self> {
        Arc::new(Self {
            log,
            shape,
            calls: Mutex::default(),
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
        let streams = match self.shape {
            Shape::Merged => tracks.len(),
            _ => tracks.iter().map(|t| t.split('|').count()).sum(),
        };
        let ids: Vec<u32> = (0..streams as u32).map(|i| 70 + i).collect();
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
fn each_source_maps_to_its_own_stream_unless_they_were_mixed_first() {
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
        Err(Error::Unsupported)
    ));
    assert!(matches!(
        targets(&[], &both, AudioSource::Mic),
        Err(Error::NotFound(_))
    ));
}

fn rig(tmp: &Path, shape: Shape) -> (GsrRecorder, Arc<Pactl>) {
    let log = tmp.join("log");
    let pactl = Pactl::new(log.clone(), shape);
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
fn separate_streams_keep_one_mixed_track_and_mute_each_stream_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, pactl) = rig(tmp.path(), Shape::PerSource);
    let mut h = recorder.start(both(tmp.path())).unwrap();
    h.set_muted(AudioSource::Mic, true).unwrap();
    h.set_muted(AudioSource::System, true).unwrap();
    h.set_muted(AudioSource::Mic, false).unwrap();
    let out = block_on(h.stop()).unwrap();
    assert_eq!(
        tracks(&read_log(&tmp.path().join("log"))),
        ["default_output|default_input"],
        "one launch, one mixed track"
    );
    assert_eq!(
        pactl.mutes(),
        [
            "set-source-output-mute 71 1",
            "set-source-output-mute 70 1",
            "set-source-output-mute 71 0"
        ]
    );
    assert!(!out.split_audio);
}

#[test]
fn one_mixed_stream_switches_to_two_tracks_that_mute_alone_and_merge_at_stop() {
    let tmp = tempfile::tempdir().unwrap();
    let (recorder, pactl) = rig(tmp.path(), Shape::Merged);
    let mut h = recorder.start(both(tmp.path())).unwrap();
    h.set_muted(AudioSource::Mic, true).unwrap();
    let out = block_on(h.stop()).unwrap();
    assert_eq!(
        tracks(&read_log(&tmp.path().join("log"))),
        [
            "default_output|default_input",
            "default_output + default_input"
        ]
    );
    assert_eq!(pactl.mutes(), ["set-source-output-mute 71 1"]);
    assert!(out.split_audio, "the two tracks still need merging");

    let again = tmp.path().join("again");
    std::fs::create_dir_all(&again).unwrap();
    let h = recorder.start(both(&again)).unwrap();
    block_on(h.stop()).unwrap();
    let all = tracks(&read_log(&tmp.path().join("log")));
    assert_eq!(
        all.last().unwrap(),
        "default_output + default_input",
        "the next recording starts on two tracks"
    );
    assert_eq!(all.len(), 3);
}

#[test]
fn without_a_sound_server_runner_nothing_is_muted() {
    let tmp = tempfile::tempdir().unwrap();
    let (_, _) = rig(tmp.path(), Shape::PerSource);
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
    let pactl = Pactl::new(tmp.path().join("missing-log"), Shape::PerSource);
    let mutes = Mutes::new(pactl.clone(), sources(true, true));
    assert!(matches!(
        mutes.set(42, AudioSource::Mic, true),
        Err(Error::NotFound(_))
    ));
    assert!(pactl.mutes().is_empty());
}

struct Ffmpeg(Mutex<Vec<Vec<String>>>);

impl CommandRunner for Ffmpeg {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        assert_eq!(program, "ffmpeg");
        self.0.lock().unwrap().push(args.to_vec());
        std::fs::write(args.last().unwrap(), b"merged")?;
        ok(String::new())
    }
}

#[test]
fn two_tracks_are_mixed_into_one_with_the_video_copied() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("screen.mkv");
    std::fs::write(&file, b"two tracks").unwrap();
    let ffmpeg = Ffmpeg(Mutex::default());
    merge_tracks(&ffmpeg, &file).unwrap();
    let merged = tmp.path().join("screen.merged.mkv");
    assert_eq!(
        ffmpeg.0.lock().unwrap()[0],
        merge_args(&file, &merged),
        "one ffmpeg run"
    );
    assert_eq!(
        merge_args(&file, &merged).join(" "),
        format!(
            "-y -v error -i {} -filter_complex [0:a:0][0:a:1]amix=inputs=2:duration=longest:normalize=0[a] \
             -map 0:v -map [a] -c:v copy -c:a libopus {}",
            file.display(),
            merged.display()
        )
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"merged",
        "the file is replaced"
    );
    assert!(!merged.exists());
    let mp4 = tmp.path().join("screen.mp4");
    assert!(merge_args(&mp4, &merged).contains(&"aac".to_owned()));
}
