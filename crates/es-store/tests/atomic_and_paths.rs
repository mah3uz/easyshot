#![allow(clippy::unwrap_used)]
use es_store::Paths;
use es_store::atomic::{ensure_private_dir, write_atomic};
use std::os::unix::fs::PermissionsExt;

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn failed_write_leaves_no_partial_file_and_no_temp() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.json");
    std::fs::create_dir_all(target.join("occupied")).unwrap();
    assert!(write_atomic(&target, b"new").is_err());
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("out.json")]);
    assert!(target.is_dir());
}

#[test]
fn replacing_a_file_leaves_only_the_new_content_and_no_temp() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.txt");
    write_atomic(&target, b"one").unwrap();
    write_atomic(&target, b"two").unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"two");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn files_are_private_and_created_directories_are_0700() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("deep/er/a.txt");
    write_atomic(&target, b"x").unwrap();
    assert_eq!(mode(&target), 0o600);
    assert_eq!(mode(target.parent().unwrap()), 0o700);
    assert_eq!(mode(&dir.path().join("deep")), 0o700);
}

#[test]
fn every_storage_location_is_created_private_under_the_root() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    for d in [
        paths.config_dir().to_path_buf(),
        paths.library_dir(),
        paths.deck_dir(),
        paths.recovery_dir(),
        paths.clipboard_dir(),
        paths.thumbs_dir(),
    ] {
        assert!(d.starts_with(dir.path()));
        assert_eq!(mode(&d), 0o700, "{d:?}");
    }
    assert_eq!(
        paths.history_index(),
        paths.library_dir().join("index.json")
    );
    assert_eq!(
        paths.bg_presets_recovery_file().file_name().unwrap(),
        "background-presets.json.recovery"
    );
    ensure_private_dir(&paths.crash_dir()).unwrap();
}
