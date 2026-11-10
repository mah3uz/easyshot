#![allow(clippy::unwrap_used)]
use es_model::CaptureKind;
use es_model::HistoryEntry;
use es_model::recording::RecordingCapture;
use es_store::Paths;
use es_store::media::{self, MediaKind};
use es_store::thumbs;
use std::cell::Cell;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn aged(path: &Path, age_secs: u64) {
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(age_secs))
        .unwrap();
}

fn video(dir: &Path, name: &str, age_secs: u64) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, b"v").unwrap();
    aged(&file, age_secs);
    file
}

fn project(paths: &Paths, name: &str, age_secs: u64, finished: bool) -> PathBuf {
    let dir = paths.recordings_dir().join(format!("{name}.easyshotrec"));
    let file = video(&dir, "screen.mkv", age_secs);
    if finished {
        let capture = RecordingCapture {
            duration_secs: 12.5,
            ..RecordingCapture::default()
        };
        es_store::recording::write(&dir, &capture).unwrap();
    }
    file
}

fn entry(file: PathBuf, age_secs: u64) -> HistoryEntry {
    HistoryEntry {
        id: uuid::Uuid::new_v4(),
        created_at: (SystemTime::now() - Duration::from_secs(age_secs)).into(),
        kind: CaptureKind::Region,
        name: "shot".into(),
        file,
        export_path: None,
        pixel_size: (8, 6),
        has_annotations: false,
        share_url: None,
    }
}

#[test]
fn the_listing_merges_images_saved_videos_and_unsaved_projects_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    video(&paths.videos_dir(), "saved.mp4", 300);
    video(&paths.videos_dir(), "notes.txt", 10);
    project(&paths, "unsaved", 100, true);
    let image = entry(dir.path().join("shot.png"), 200);

    let listed = media::list(&paths, &[image]);

    let seen: Vec<_> = listed.iter().map(|m| (m.name.as_str(), m.kind)).collect();
    assert_eq!(
        seen,
        [
            ("unsaved", MediaKind::Video),
            ("shot", MediaKind::Image),
            ("saved.mp4", MediaKind::Video),
        ]
    );
}

#[test]
fn an_unsaved_project_shows_its_duration_and_one_still_recording_is_not_listed() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let done = project(&paths, "done", 10, true);
    project(&paths, "live", 5, false);

    let found = media::videos(&paths);

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].file, done);
    assert_eq!(found[0].duration_secs, Some(12.5));
    assert_eq!(found[0].project.as_deref(), done.parent());
}

fn grab(calls: &Cell<u32>) -> impl FnOnce(&Path) -> Option<Vec<u8>> + '_ {
    |_| {
        calls.set(calls.get() + 1);
        let mut png = Vec::new();
        image::RgbaImage::new(64, 36)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        Some(png)
    }
}

#[test]
fn a_video_thumbnail_is_made_once_and_made_again_when_the_file_changes() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let file = video(&paths.videos_dir(), "a.mp4", 100);
    let calls = Cell::new(0);

    let first = thumbs::ensure_video(&paths, &file, 32, grab(&calls)).unwrap();
    let again = thumbs::ensure_video(&paths, &file, 32, grab(&calls)).unwrap();
    assert_eq!((first.as_path(), calls.get()), (again.as_path(), 1));

    aged(&file, 5);
    let edited = thumbs::ensure_video(&paths, &file, 32, grab(&calls)).unwrap();
    assert_ne!(edited, first);
    assert_eq!(calls.get(), 2);
}

#[test]
fn a_missing_frame_source_is_an_error_not_a_thumbnail() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let file = video(&paths.videos_dir(), "a.mp4", 100);
    assert!(thumbs::ensure_video(&paths, &file, 32, |_| None).is_err());
}
