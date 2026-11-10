#![allow(clippy::unwrap_used)]

use es_model::recording::{PressSample, RecordingCapture, RecordingInput, RecordingProject};
use es_record::recovery::{parse_probe, probe_video};
use es_record::{ProjectFolder, SystemRunner, recover};
use es_store::recording::RecordingFile;
use std::path::Path;
use std::process::Command;
use time::OffsetDateTime;

fn make_clip(path: &Path, secs: &str) -> bool {
    Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-threads", "2", "-f", "lavfi", "-i"])
        .arg("testsrc=size=320x240:rate=10")
        .args(["-t", secs, "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(path)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .status()
        .is_ok_and(|s| s.success())
}

fn read<T: RecordingFile>(dir: &Path) -> T {
    let file = es_store::recording::read::<T>(dir).unwrap().unwrap();
    assert_eq!(file.newer_version(), None);
    file.value
}

#[test]
fn ffprobe_json_parses_to_a_probe() {
    let p = parse_probe(
        r#"{"streams":[{"codec_name":"h264","width":320,"height":240}],"format":{"duration":"1.000000"}}"#,
    )
    .unwrap();
    assert_eq!((p.width, p.height, p.codec.as_str()), (320, 240, "h264"));
    assert_eq!(p.duration_secs, 1.0);
    assert!(parse_probe(r#"{"streams":[],"format":{}}"#).is_none());
}

#[test]
fn a_session_without_capture_json_but_with_playable_footage_is_recovered() {
    let root = tempfile::tempdir().unwrap();
    let crashed = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    if !make_clip(&crashed.video_path(), "1") {
        eprintln!("skipped: ffmpeg cannot generate the test clip");
        return;
    }
    let finished = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    finished
        .write_capture(&RecordingCapture::default())
        .unwrap();
    let broken = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    std::fs::write(broken.video_path(), b"not a movie").unwrap();

    let report = recover(root.path(), &SystemRunner).unwrap();

    assert_eq!(report.recovered.len(), 1);
    assert_eq!(report.recovered[0].dir, crashed.dir());
    assert_eq!(report.unplayable, [broken.dir().to_path_buf()]);
    assert!(
        broken.video_path().exists(),
        "unplayable footage is never deleted"
    );
    assert!(!broken.has_capture() && finished.has_capture());

    let cap = read::<RecordingCapture>(crashed.dir());
    assert_eq!((cap.pixel_width, cap.pixel_height), (320, 240));
    assert!(
        (cap.duration_secs - 1.0).abs() < 0.2,
        "{}",
        cap.duration_secs
    );
    let proj = read::<RecordingProject>(crashed.dir());
    assert!(proj.display_name.starts_with("EasyShot_"));
    assert!(crashed.dir().join("poster.jpg").is_file());

    let again = recover(root.path(), &SystemRunner).unwrap();
    assert!(
        again.recovered.is_empty(),
        "a recovered session is not recovered twice"
    );
}

#[test]
fn a_crash_while_finishing_keeps_the_files_already_written_and_adds_capture_json() {
    let root = tempfile::tempdir().unwrap();
    let crashed = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    if !make_clip(&crashed.video_path(), "1") {
        eprintln!("skipped: ffmpeg cannot generate the test clip");
        return;
    }
    let input = RecordingInput {
        presses: vec![PressSample {
            time: 0.5,
            button: 272,
            ..PressSample::default()
        }],
        ..RecordingInput::default()
    };
    crashed.write_input(&input).unwrap();
    crashed
        .write_project(&RecordingProject {
            display_name: "named before the crash".into(),
            ..RecordingProject::default()
        })
        .unwrap();
    assert!(!crashed.has_capture());

    let report = recover(root.path(), &SystemRunner).unwrap();

    assert_eq!(report.recovered.len(), 1);
    let cap = read::<RecordingCapture>(crashed.dir());
    assert_eq!((cap.pixel_width, cap.pixel_height), (320, 240));
    assert_eq!(
        read::<RecordingProject>(crashed.dir()).display_name,
        "named before the crash"
    );
    assert_eq!(read::<RecordingInput>(crashed.dir()), input);
}

#[test]
fn footage_shorter_than_a_tenth_of_a_second_is_not_recovered() {
    let root = tempfile::tempdir().unwrap();
    let f = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    if !make_clip(&f.video_path(), "0.05") {
        eprintln!("skipped: ffmpeg cannot generate the test clip");
        return;
    }
    assert!(probe_video(&SystemRunner, &f.video_path()).is_some());
    let report = recover(root.path(), &SystemRunner).unwrap();
    assert!(report.recovered.is_empty() && report.unplayable.len() == 1);
}

#[test]
fn a_missing_recordings_dir_recovers_nothing() {
    let root = tempfile::tempdir().unwrap();
    let r = recover(&root.path().join("nope"), &SystemRunner).unwrap();
    assert!(r.recovered.is_empty() && r.unplayable.is_empty());
}
