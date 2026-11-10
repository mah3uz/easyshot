#![cfg(feature = "player")]
#![allow(clippy::unwrap_used)]
use crate::window_mode_support::Surface;

use es_overlays::player::engine::{Engine, Signal};
use es_overlays::player::{
    AudioOut, ExportJob, ExportMode, ExportOptions, PlayerConfig, PlayerWindow, SystemFfmpeg, Trim,
    export::export,
};
use es_record::SystemRunner;
use es_store::Paths;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

fn ffmpeg(args: &[&str]) -> std::process::Output {
    Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-nostats"])
        .args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap()
}

/// Two seconds of bars; a 440 Hz track first and a 1000 Hz track second, both at the same level.
fn two_tone_video(dir: &Path, tracks: usize) -> PathBuf {
    let out = dir.join(format!("tones{tracks}.mkv"));
    let mut args = vec![
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x180:rate=30:duration=2",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=2",
    ];
    if tracks == 2 {
        args.extend(["-f", "lavfi", "-i", "sine=frequency=1000:duration=2"]);
    }
    args.extend(["-map", "0:v", "-map", "1:a"]);
    if tracks == 2 {
        args.extend(["-map", "2:a"]);
    }
    args.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"]);
    let out_s = out.to_string_lossy().into_owned();
    args.push(&out_s);
    let made = ffmpeg(&args);
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    out
}

fn mean_db(file: &Path, hz: u32) -> f64 {
    let filter = format!("bandpass=f={hz}:width_type=h:w=60,volumedetect");
    let out = Command::new("ffmpeg")
        .args(["-v", "info", "-nostats", "-i"])
        .arg(file)
        .args(["-vn", "-af", &filter, "-f", "null", "-"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .find_map(|l| l.split("mean_volume:").nth(1))
        .and_then(|v| v.trim().trim_end_matches(" dB").parse().ok())
        .unwrap()
}

fn audio_streams(file: &Path) -> usize {
    es_record::recovery::audio_track_count(&SystemRunner, file)
}

fn job(input: &Path, output: PathBuf, tracks: usize, options: ExportOptions) -> ExportJob {
    ExportJob {
        input: input.to_path_buf(),
        output,
        trim: Trim::new(2.),
        mode: ExportMode::Precise,
        remove_audio: false,
        audio_tracks: tracks,
        options,
    }
}

#[test]
fn an_export_of_two_tracks_has_one_audio_stream_with_both_tones_at_the_chosen_levels() {
    let dir = tempfile::tempdir().unwrap();
    let video = two_tone_video(dir.path(), 2);
    for (name, system, mic, expect_db) in [
        ("half.mp4", 0.5, 1.0, -6.0),
        ("equal.mp4", 1.0, 1.0, 0.0),
        ("quiet.mp4", 0.25, 1.0, -12.0),
    ] {
        let options = ExportOptions {
            system_volume: system,
            mic_volume: mic,
            ..Default::default()
        };
        let out = export(
            &SystemFfmpeg,
            &job(&video, dir.path().join(name), 2, options),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(audio_streams(&out), 1, "{name}");
        let (system_db, mic_db) = (mean_db(&out, 440), mean_db(&out, 1000));
        assert!(
            (system_db - mic_db - expect_db).abs() < 1.5,
            "{name}: system {system_db} dB, mic {mic_db} dB, wanted {expect_db} dB apart"
        );
    }
}

#[test]
fn an_export_of_one_track_is_not_mixed() {
    let dir = tempfile::tempdir().unwrap();
    let video = two_tone_video(dir.path(), 1);
    let out = export(
        &SystemFfmpeg,
        &job(
            &video,
            dir.path().join("one.mp4"),
            1,
            ExportOptions::default(),
        ),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(audio_streams(&out), 1);
}

#[test]
fn playback_builds_one_volume_per_track_and_follows_live_changes() {
    let dir = tempfile::tempdir().unwrap();
    let video = two_tone_video(dir.path(), 2);
    let engine = Engine::open(&video, AudioOut::Silent, (320, 180), &[0.5, 1.]).unwrap();
    let rx = engine.signals();
    loop {
        match rx.recv_blocking().unwrap() {
            Signal::Settled => break,
            Signal::Failed(e) => panic!("{e}"),
            _ => {}
        }
    }
    assert_eq!(engine.audio_tracks(), 2);
    assert_eq!((engine.level(0), engine.level(1)), (Some(0.5), Some(1.)));
    engine.set_level(1, 1.5);
    assert_eq!(engine.level(1), Some(1.5));
    engine.play();
    while !matches!(rx.recv_blocking().unwrap(), Signal::End) {}
}

fn open(dir: &Path, video: &Path) -> Surface<PlayerWindow> {
    let config = PlayerConfig {
        video: video.to_path_buf(),
        output: {
            let dir = dir.to_path_buf();
            std::rc::Rc::new(move |ext| dir.join(format!("out.{ext}")))
        },
        options: ExportOptions::default(),
        audio: AudioOut::Silent,
        paths: Paths::rooted(&dir.join("root")),
        commands: Arc::new(SystemRunner),
        ffmpeg: Arc::new(SystemFfmpeg),
    };
    Surface::open(960., 600., move |cx| PlayerWindow::new(config, cx))
}

#[test]
fn a_two_track_video_shows_two_volume_sliders_system_lower_a_one_track_video_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut two = open(dir.path(), &two_tone_video(dir.path(), 2));
    let view = two.view.clone();
    two.app.until("the sliders", |cx| {
        view.read(cx).volume_sliders(cx).is_some()
    });
    assert_eq!(
        two.app.cx.update(|cx| view.read(cx).volume_sliders(cx)),
        Some([0.5, 1.])
    );

    let mut one = open(dir.path(), &two_tone_video(dir.path(), 1));
    let view = one.view.clone();
    one.app
        .until("the video", |cx| view.read(cx).trim().duration() > 0.);
    one.app.settle();
    assert_eq!(
        one.app.cx.update(|cx| view.read(cx).volume_sliders(cx)),
        None
    );
}
