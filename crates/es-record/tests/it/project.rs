#![allow(clippy::unwrap_used)]

use es_model::recording::{RecordingCapture, RecordingInput, RecordingProject};
use es_record::project::{CAPTURE, INPUT, POSTER, PROJECT, folder_name, poster_time};
use es_record::{CommandRunner, ProjectFolder};
use std::process::Output;
use std::sync::Mutex;
use time::OffsetDateTime;

#[derive(Default)]
struct Spy(Mutex<Vec<(String, Vec<String>)>>);

impl CommandRunner for Spy {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        self.0
            .lock()
            .unwrap()
            .push((program.to_owned(), args.to_vec()));
        std::fs::write(args.last().unwrap(), b"jpg")?;
        Ok(Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

#[test]
fn the_folder_name_is_unique_and_carries_the_suffix() {
    let a = folder_name(OffsetDateTime::UNIX_EPOCH);
    assert!(a.starts_with("EasyShot_1970-01-01-000000_") && a.ends_with(".easyshotrec"));
    assert_ne!(a, folder_name(OffsetDateTime::UNIX_EPOCH));
}

#[test]
fn the_poster_is_taken_at_one_second_or_half_a_short_clip() {
    assert_eq!(poster_time(10.0), 1.0);
    assert_eq!(poster_time(0.6), 0.3);
}

#[test]
fn finishing_writes_every_file_and_capture_json_last() {
    let root = tempfile::tempdir().unwrap();
    let folder = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    std::fs::write(folder.video_path(), b"mkv").unwrap();
    assert!(!folder.has_capture());
    let spy = Spy::default();
    folder
        .finish(
            &RecordingCapture {
                duration_secs: 0.6,
                ..RecordingCapture::default()
            },
            &RecordingProject::default(),
            &RecordingInput::default(),
            &spy,
        )
        .unwrap();
    for f in [CAPTURE, INPUT, PROJECT, POSTER, "screen.mkv"] {
        assert!(folder.dir().join(f).is_file(), "{f}");
    }
    let calls = spy.0.lock().unwrap();
    assert_eq!(calls[0].0, "ffmpeg");
    let ss = calls[0].1.iter().position(|a| a == "-ss").unwrap();
    assert_eq!(calls[0].1[ss + 1], "0.300");
    let names: Vec<_> = std::fs::read_dir(folder.dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.starts_with(".easyshot-tmp-")),
        "{names:?}"
    );
}

#[test]
fn a_failed_poster_does_not_stop_the_manifest_being_written() {
    struct Failing;
    impl CommandRunner for Failing {
        fn run(&self, _: &str, _: &[String]) -> std::io::Result<Output> {
            Err(std::io::Error::other("no ffmpeg"))
        }
    }
    let root = tempfile::tempdir().unwrap();
    let folder = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    folder
        .finish(
            &RecordingCapture::default(),
            &RecordingProject::default(),
            &RecordingInput::default(),
            &Failing,
        )
        .unwrap();
    assert!(folder.has_capture());
    assert!(!folder.dir().join(POSTER).exists());
}

#[test]
fn an_existing_manifest_is_replaced_whole_never_truncated() {
    let root = tempfile::tempdir().unwrap();
    let folder = ProjectFolder::create(root.path(), OffsetDateTime::UNIX_EPOCH).unwrap();
    for secs in [1.0, 2.0] {
        folder
            .write_capture(&RecordingCapture {
                duration_secs: secs,
                ..RecordingCapture::default()
            })
            .unwrap();
    }
    let back: RecordingCapture =
        serde_json::from_slice(&std::fs::read(folder.dir().join(CAPTURE)).unwrap()).unwrap();
    assert_eq!(back.duration_secs, 2.0);
}
