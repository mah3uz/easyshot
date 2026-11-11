#![allow(clippy::unwrap_used)]
use es_store::Paths;
use es_store::temp::{TempKind, create, make_dir, purge};
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, SystemTime};

fn setup() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    (dir, paths)
}

#[test]
fn the_file_keeps_its_name_inside_a_private_uuid_directory() {
    let (_d, paths) = setup();
    let file = create(&paths, TempKind::Drag, "My Shot.png", b"x").unwrap();
    assert_eq!(file.file_name().unwrap(), "My Shot.png");
    assert_eq!(file.parent().unwrap().parent().unwrap(), paths.drag_dir());
    let mode = std::fs::metadata(file.parent().unwrap())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn a_name_cannot_escape_its_directory() {
    let (_d, paths) = setup();
    let file = create(&paths, TempKind::Clipboard, "../../evil.png", b"x").unwrap();
    assert!(file.starts_with(paths.clipboard_dir()));
    assert_eq!(file.file_name().unwrap(), "evil.png");
}

#[test]
fn purge_removes_day_old_items_and_leaves_fresh_ones() {
    let (_d, paths) = setup();
    let old = create(&paths, TempKind::Clipboard, "old.png", b"x").unwrap();
    let old_dir = old.parent().unwrap().to_path_buf();
    std::fs::File::open(&old_dir)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(25 * 3600))
        .unwrap();
    let fresh = create(&paths, TempKind::Unstaged, "fresh.png", b"x").unwrap();
    let removed = purge(&paths, SystemTime::now()).unwrap();
    assert_eq!(removed, 1);
    assert!(!old_dir.exists());
    assert!(fresh.exists());
}

#[test]
fn a_share_dir_is_under_the_cache_and_purged_when_a_day_old() {
    let (_d, paths) = setup();
    let dir = make_dir(&paths, TempKind::Share).unwrap();
    assert_eq!(dir.parent().unwrap(), paths.share_dir());
    std::fs::File::open(&dir)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(25 * 3600))
        .unwrap();
    assert_eq!(purge(&paths, SystemTime::now()).unwrap(), 1);
    assert!(!dir.exists());
}
