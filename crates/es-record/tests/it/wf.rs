#![allow(clippy::unwrap_used)]
use crate::common;

use common::*;
use es_record::wf::{Encoder, concat_list, pick_encoder, wf_args};
use es_record::{Codec, Error, Recorder, RecorderEvent, WfRecorder};
use futures::executor::block_on;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FULL: &str =
    " V....D h264_vaapi  H.264 (VAAPI)\n V....D libx264  libx264\n V....D libvpx-vp9  VP9\n";
const NO_X264: &str = " V....D libvpx-vp9  VP9\n V....D libopenh264  OpenH264\n";

fn h264(text: &str, vaapi: bool) -> Result<Encoder, Error> {
    pick_encoder(text, Codec::H264, vaapi)
}

#[test]
fn the_encoder_falls_back_from_vaapi_to_x264_to_vp9() {
    assert_eq!(h264(FULL, true).unwrap().name, "h264_vaapi");
    assert_eq!(h264(FULL, false).unwrap().name, "libx264");
    assert_eq!(h264(NO_X264, true).unwrap().name, "libvpx-vp9");
    assert!(matches!(h264("", true), Err(Error::Unsupported)));
}

#[test]
fn an_encoder_name_must_match_a_whole_token() {
    assert!(h264(" V..... libx264rgb  x", false).is_err());
}

#[test]
fn a_region_is_passed_as_x_y_size_in_logical_pixels() {
    let s = spec(Path::new("/r"), region("DP-2", 3083.0, 382.0, 800.0, 600.0));
    let enc = h264(FULL, true).unwrap();
    let a = wf_args(
        &s,
        &enc,
        Some(Path::new("/dev/dri/renderD128")),
        None,
        Path::new("/r/s.mkv"),
    )
    .unwrap()
    .join(" ");
    assert_eq!(
        a,
        "-f /r/s.mkv -r 60 -c h264_vaapi -d /dev/dri/renderD128 -g 3083,382 800x600"
    );
}

#[test]
fn an_output_uses_dash_o_and_software_encoding_needs_no_device() {
    let s = spec(Path::new("/r"), output("DP-2"));
    let enc = h264(FULL, false).unwrap();
    let a = wf_args(
        &s,
        &enc,
        Some(Path::new("/dev/dri/renderD128")),
        None,
        Path::new("/r/s.mkv"),
    )
    .unwrap()
    .join(" ");
    assert_eq!(a, "-f /r/s.mkv -r 60 -c libx264 -o DP-2");
}

#[test]
fn one_audio_track_is_supported_and_two_are_refused() {
    let enc = h264(FULL, false).unwrap();
    let mut s = spec(Path::new("/r"), output("DP-2"));
    s.mic = Some("alsa_input.x".into());
    let a = wf_args(&s, &enc, None, None, Path::new("/f"))
        .unwrap()
        .join(" ");
    assert!(a.contains("--audio=alsa_input.x"));
    s.system_audio = true;
    assert!(matches!(
        wf_args(&s, &enc, None, Some("mon"), Path::new("/f")),
        Err(Error::Unsupported)
    ));
    s.mic = None;
    assert!(matches!(
        wf_args(&s, &enc, None, None, Path::new("/f")),
        Err(Error::Unsupported)
    ));
    let a = wf_args(&s, &enc, None, Some("mon.monitor"), Path::new("/f"))
        .unwrap()
        .join(" ");
    assert!(a.contains("--audio=mon.monitor"));
}

#[test]
fn the_concat_list_quotes_paths() {
    let l = concat_list(&[PathBuf::from("/a/it's.mkv"), PathBuf::from("/a/b.mkv")]);
    assert_eq!(l, "file '/a/it'\\''s.mkv'\nfile '/a/b.mkv'\n");
}

struct Rig {
    tmp: tempfile::TempDir,
}

impl Rig {
    fn recorder(&self) -> WfRecorder {
        WfRecorder::new(FULL.into(), None)
            .with_programs(fake("fake-wf"), fake("fake-ffmpeg"))
            .with_clock(Arc::new(FakeClock::new()))
            .with_env("FAKE_LOG", &self.tmp.path().join("log").to_string_lossy())
    }

    fn lines(&self) -> Vec<String> {
        read_log(&self.tmp.path().join("log"))
    }
}

#[test]
fn pausing_records_segments_that_are_joined_with_an_ffmpeg_concat_on_stop() {
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let s = spec(rig.tmp.path(), output("DP-2"));
    let dir = s.dir.clone();
    let mut h = rig.recorder().start(s).unwrap();
    let events = h.events();
    h.pause().unwrap();
    h.resume().unwrap();
    let out = block_on(h.stop()).unwrap();

    let log = rig.lines();
    let wf: Vec<_> = log.iter().filter(|l| l.starts_with("argv")).collect();
    assert_eq!(wf.len(), 2);
    assert!(wf[0].contains(&format!("-f {}/segment-000.mkv", dir.display())));
    assert!(wf[1].contains(&format!("-f {}/segment-001.mkv", dir.display())));
    let ff = log.iter().find(|l| l.starts_with("ffmpeg")).unwrap();
    assert_eq!(
        *ff,
        format!(
            "ffmpeg -y -f concat -safe 0 -i {d}/segments.txt -c copy {d}/screen.mkv",
            d = dir.display()
        )
    );
    assert_eq!(out.file, dir.join("screen.mkv"));
    assert_eq!(out.segments.len(), 2);
    assert!(!dir.join("segment-000.mkv").exists() && !dir.join("segments.txt").exists());
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
fn an_unpaused_recording_is_renamed_without_running_ffmpeg() {
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let s = spec(rig.tmp.path(), output("DP-2"));
    let dir = s.dir.clone();
    let h = rig.recorder().start(s).unwrap();
    let out = block_on(h.stop()).unwrap();
    assert_eq!(out.file, dir.join("screen.mkv"));
    assert!(out.file.is_file());
    assert!(!rig.lines().iter().any(|l| l.starts_with("ffmpeg")));
}

#[test]
fn discard_removes_the_folder() {
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let s = spec(rig.tmp.path(), output("DP-2"));
    let dir = s.dir.clone();
    let h = rig.recorder().start(s).unwrap();
    block_on(h.discard()).unwrap();
    assert!(!dir.exists());
}

#[test]
fn system_sound_reaches_the_command_line_only_when_a_monitor_is_known() {
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let mut s = spec(rig.tmp.path(), output("DP-2"));
    s.system_audio = true;
    let with = rig.recorder().with_system_audio_device("out.monitor");
    assert!(with.can_record_system_audio(false));
    assert!(!with.can_record_system_audio(true));
    block_on(with.start(s.clone()).unwrap().stop()).unwrap();
    assert!(
        rig.lines()
            .iter()
            .any(|l| l.contains("--audio=out.monitor"))
    );

    let without = rig.recorder();
    assert!(!without.can_record_system_audio(false));
    s.system_audio = false;
    block_on(without.start(s).unwrap().stop()).unwrap();
    let audio_flags = rig.lines().iter().filter(|l| l.contains("--audio")).count();
    assert_eq!(audio_flags, 1, "the second recording has no audio flag");
}

struct OneStream {
    log: PathBuf,
    calls: std::sync::Mutex<Vec<String>>,
}

impl es_record::CommandRunner for OneStream {
    fn run(&self, _: &str, args: &[String]) -> std::io::Result<std::process::Output> {
        use std::os::unix::process::ExitStatusExt as _;
        self.calls.lock().unwrap().push(args.join(" "));
        let pids: Vec<String> = read_log(&self.log)
            .iter()
            .filter_map(|l| l.strip_prefix("pid ").map(str::to_owned))
            .collect();
        let json = match pids.last() {
            Some(pid) if args[0] == "-f" => format!(
                r#"[{{"index":{},"properties":{{"application.process.id":"{pid}"}}}}]"#,
                70 + pids.len()
            ),
            _ => "[]".into(),
        };
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: json.into_bytes(),
            stderr: Vec::new(),
        })
    }
}

#[test]
fn the_one_source_is_muted_on_the_current_segments_stream_and_again_after_a_resume() {
    use es_record::AudioSource;
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let pactl = Arc::new(OneStream {
        log: rig.tmp.path().join("log"),
        calls: Default::default(),
    });
    let mut s = spec(rig.tmp.path(), output("DP-2"));
    s.system_audio = true;
    let recorder = rig
        .recorder()
        .with_system_audio_device("out.monitor")
        .with_audio_runner(pactl.clone());
    let mut h = recorder.start(s).unwrap();
    h.set_muted(AudioSource::System, true).unwrap();
    h.pause().unwrap();
    h.resume().unwrap();
    block_on(h.stop()).unwrap();
    let mutes: Vec<String> = pactl
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.starts_with("set-source-output-mute"))
        .cloned()
        .collect();
    assert_eq!(
        mutes,
        ["set-source-output-mute 71 1", "set-source-output-mute 72 1"],
        "each wf-recorder process is a new stream"
    );
}

#[test]
fn without_a_sound_server_runner_wf_recorder_mutes_nothing() {
    use es_record::AudioSource;
    let rig = Rig {
        tmp: tempfile::tempdir().unwrap(),
    };
    let mut s = spec(rig.tmp.path(), output("DP-2"));
    s.system_audio = true;
    let recorder = rig.recorder().with_system_audio_device("out.monitor");
    let mut h = recorder.start(s).unwrap();
    assert!(matches!(
        h.set_muted(AudioSource::System, true),
        Err(Error::Unsupported)
    ));
    block_on(h.stop()).unwrap();
}
