#![allow(clippy::unwrap_used)]
use es_store::document::sidecar_path;
use es_store::library::base_path;
use es_store::thumbs::thumb_path;
use es_store::{History, Paths};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn png(path: &Path, w: u32, h: u32, age_secs: u64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbaImage::new(w, h).save(path).unwrap();
    let when = SystemTime::now() - Duration::from_secs(age_secs);
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

fn capture(paths: &Paths, name: &str, age_secs: u64) -> PathBuf {
    let image = paths.library_dir().join(format!("{name}.png"));
    png(&image, 8, 6, age_secs);
    std::fs::write(sidecar_path(&image), "{}").unwrap();
    png(&base_path(&image), 8, 6, age_secs);
    image
}

fn names(history: &History) -> Vec<String> {
    history.entries().iter().map(|e| e.name.clone()).collect()
}

fn setup() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    (dir, paths)
}

#[test]
fn index_is_rebuilt_from_the_library_when_missing() {
    let (_d, paths) = setup();
    capture(&paths, "old", 300);
    capture(&paths, "new", 100);
    let history = History::open(&paths, 100).unwrap();
    assert_eq!(names(&history), ["new", "old"]);
    let new = &history.entries()[0];
    assert_eq!(new.pixel_size, (8, 6));
    assert!(new.has_annotations);
    assert_eq!(history.entries().len(), 2, "base images are not captures");
}

#[test]
fn a_corrupt_index_is_replaced_by_what_the_library_holds() {
    let (_d, paths) = setup();
    capture(&paths, "a", 100);
    std::fs::write(paths.history_index(), "{ not json").unwrap();
    let history = History::open(&paths, 100).unwrap();
    assert_eq!(names(&history), ["a"]);
    drop(history);
    let again = History::open(&paths, 100).unwrap();
    assert_eq!(
        names(&again),
        ["a"],
        "the rebuilt index was written and parses"
    );
}

#[test]
fn entries_whose_file_was_deleted_are_dropped_on_open() {
    let (_d, paths) = setup();
    let a = capture(&paths, "a", 200);
    capture(&paths, "b", 100);
    let history = History::open(&paths, 100).unwrap();
    drop(history);
    std::fs::remove_file(&a).unwrap();
    let history = History::open(&paths, 100).unwrap();
    assert_eq!(names(&history), ["b"]);
}

#[test]
fn rebuilt_ids_are_stable() {
    let (_d, paths) = setup();
    capture(&paths, "a", 100);
    let first = History::open(&paths, 100).unwrap().entries()[0].id;
    std::fs::remove_file(paths.history_index()).unwrap();
    let second = History::open(&paths, 100).unwrap().entries()[0].id;
    assert_eq!(first, second);
}

#[test]
fn retention_removes_exactly_the_oldest_beyond_the_limit_with_all_three_files() {
    let (_d, paths) = setup();
    let oldest = capture(&paths, "c1", 500);
    let older = capture(&paths, "c2", 400);
    let keep_a = capture(&paths, "c3", 300);
    let keep_b = capture(&paths, "c4", 200);
    let mut history = History::open(&paths, 0).unwrap();
    assert_eq!(history.entries().len(), 4);

    let gone = history.set_limit(2).unwrap();
    let mut gone_names: Vec<_> = gone.iter().map(|e| e.name.as_str()).collect();
    gone_names.sort();
    assert_eq!(gone_names, ["c1", "c2"]);
    assert_eq!(names(&history), ["c4", "c3"]);
    for image in [&oldest, &older] {
        assert!(!image.exists());
        assert!(!sidecar_path(image).exists());
        assert!(!base_path(image).exists());
    }
    for image in [&keep_a, &keep_b] {
        assert!(image.exists() && sidecar_path(image).exists() && base_path(image).exists());
    }
}

#[test]
fn adding_beyond_the_limit_evicts_the_oldest_not_the_newcomer() {
    let (_d, paths) = setup();
    capture(&paths, "a", 300);
    capture(&paths, "b", 200);
    let mut history = History::open(&paths, 2).unwrap();
    let fresh = capture(&paths, "fresh", 0);
    let entry = es_store::library::entry_for_file(&fresh).unwrap();
    let gone = history.add(entry).unwrap();
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].name, "a");
    assert_eq!(names(&history), ["fresh", "b"]);
}

#[test]
fn retention_never_deletes_a_capture_saved_outside_the_library() {
    let (dir, paths) = setup();
    let outside = dir.path().join("Pictures").join("shot.png");
    png(&outside, 4, 4, 900);
    let mut history = History::open(&paths, 1).unwrap();
    let mut entry = es_store::library::entry_for_file(&outside).unwrap();
    entry.name = "outside".into();
    history.add(entry).unwrap();
    capture(&paths, "inside", 10);
    let history = History::open(&paths, 1).unwrap();
    assert!(outside.exists(), "a referenced file belongs to the user");
    assert_eq!(names(&history), ["inside"]);
}

#[test]
fn remove_and_rename() {
    let (_d, paths) = setup();
    let a = capture(&paths, "a", 100);
    let mut history = History::open(&paths, 100).unwrap();
    let id = history.entries()[0].id;
    assert!(history.rename(id, "Renamed").unwrap());
    assert_eq!(history.get(id).unwrap().name, "Renamed");
    assert_eq!(history.by_file(&a).unwrap().id, id);
    assert!(history.remove(id).unwrap());
    assert!(!a.exists() && !sidecar_path(&a).exists() && !base_path(&a).exists());
    assert!(!history.remove(id).unwrap());
    let reopened = History::open(&paths, 100).unwrap();
    assert!(reopened.entries().is_empty());
}

#[test]
fn a_newer_index_is_never_written() {
    let (_d, paths) = setup();
    capture(&paths, "a", 100);
    let newer = r#"{"format":"easyshot.history","version":99,"entries":[]}"#;
    std::fs::write(paths.history_index(), newer).unwrap();
    let mut history = History::open(&paths, 100).unwrap();
    let id = history.entries()[0].id;
    history.rename(id, "x").unwrap();
    assert_eq!(
        std::fs::read_to_string(paths.history_index()).unwrap(),
        newer
    );
}

#[test]
fn thumbnail_path_follows_path_mtime_and_size() {
    let (_d, paths) = setup();
    let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
    let a = thumb_path(&paths, Path::new("/x/a.png"), t0, 120);
    assert_eq!(a, thumb_path(&paths, Path::new("/x/a.png"), t0, 120));
    assert_ne!(a, thumb_path(&paths, Path::new("/x/b.png"), t0, 120));
    assert_ne!(
        a,
        thumb_path(
            &paths,
            Path::new("/x/a.png"),
            t0 + Duration::from_secs(1),
            120
        )
    );
    assert_ne!(a, thumb_path(&paths, Path::new("/x/a.png"), t0, 240));
    let file = a.file_name().unwrap().to_str().unwrap();
    assert!(file.ends_with("-120.png") && file.len() == 40 + "-120.png".len());
    assert_eq!(a.parent().unwrap(), paths.thumbs_dir());
}
