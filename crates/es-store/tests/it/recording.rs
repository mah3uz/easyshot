#![allow(clippy::unwrap_used)]
use es_model::recording::{PressSample, RecordingCapture, RecordingInput, RecordingProject};
use es_store::Error;
use es_store::recording::{self, CAPTURE, INPUT, PROJECT};
use std::path::Path;
use time::OffsetDateTime;

fn on_disk(dir: &Path, name: &str) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(dir.join(name)).unwrap()).unwrap()
}

#[test]
fn each_recording_file_round_trips_through_its_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let capture = RecordingCapture {
        duration_secs: 4.5,
        pixel_width: 1920,
        ..RecordingCapture::default()
    };
    let input = RecordingInput {
        presses: vec![PressSample {
            time: 1.25,
            button: 272,
            ..PressSample::default()
        }],
        ..RecordingInput::default()
    };
    let project = RecordingProject {
        display_name: "demo".into(),
        saved_at: OffsetDateTime::UNIX_EPOCH,
        ..RecordingProject::default()
    };
    recording::write(dir.path(), &capture).unwrap();
    recording::write(dir.path(), &input).unwrap();
    recording::write(dir.path(), &project).unwrap();

    for (name, format, version) in [
        (CAPTURE, "easyshot.capture", 2),
        (INPUT, "easyshot.input", 1),
        (PROJECT, "easyshot.project", 1),
    ] {
        let v = on_disk(dir.path(), name);
        assert_eq!(
            (v["format"].as_str(), v["version"].as_u64()),
            (Some(format), Some(version))
        );
    }
    let back = recording::read::<RecordingCapture>(dir.path())
        .unwrap()
        .unwrap();
    assert_eq!((back.newer_version(), back.value), (None, capture));
    assert_eq!(
        recording::read::<RecordingInput>(dir.path())
            .unwrap()
            .unwrap()
            .value,
        input
    );
    assert_eq!(
        recording::read::<RecordingProject>(dir.path())
            .unwrap()
            .unwrap()
            .value,
        project
    );
}

#[test]
fn a_folder_without_the_file_reads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        recording::read::<RecordingCapture>(dir.path())
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_file_without_the_envelope_does_not_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(CAPTURE), r#"{"duration_secs":3.0}"#).unwrap();
    assert!(recording::read::<RecordingCapture>(dir.path()).is_err());
}

#[test]
fn a_newer_file_opens_read_only_and_is_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let newer = r#"{"format":"easyshot.capture","version":9,"duration_secs":7.0,"future":1}"#;
    std::fs::write(dir.path().join(CAPTURE), newer).unwrap();

    let opened = recording::read::<RecordingCapture>(dir.path())
        .unwrap()
        .unwrap();
    assert_eq!(opened.newer_version(), Some(9));
    assert_eq!(opened.value.duration_secs, 7.0);
    assert!(matches!(
        opened.save(),
        Err(Error::NewerVersion { version: 9 })
    ));
    assert!(matches!(
        recording::write(dir.path(), &RecordingCapture::default()),
        Err(Error::NewerVersion { version: 9 })
    ));
    assert_eq!(
        std::fs::read_to_string(dir.path().join(CAPTURE)).unwrap(),
        newer
    );
}

#[test]
fn a_write_replaces_the_file_whole_and_leaves_no_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    for secs in [1.0, 2.0] {
        let capture = RecordingCapture {
            duration_secs: secs,
            ..RecordingCapture::default()
        };
        recording::write(dir.path(), &capture).unwrap();
    }
    assert_eq!(on_disk(dir.path(), CAPTURE)["duration_secs"], 2.0);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, [CAPTURE]);
}

#[test]
fn a_write_into_a_removed_folder_fails_and_does_not_make_it_again() {
    let root = tempfile::tempdir().unwrap();
    let gone = root.path().join("discarded.easyshotrec");
    assert!(recording::write(&gone, &RecordingCapture::default()).is_err());
    assert!(!gone.exists());
}
