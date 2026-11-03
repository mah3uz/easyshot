#![allow(clippy::unwrap_used)]
use es_store::{ConfigFile, Error, FileWatcher};
use std::sync::mpsc;
use std::time::Duration;

const HAND_WRITTEN: &str =
    "# my notes\n[general]\ntheme = \"dark\" # keep\nplay_sound = true\n\n[mystery]\nvalue = 3\n";

fn write(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let p = dir.join("config.toml");
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn save_changes_only_the_edited_key_and_keeps_comments_and_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), HAND_WRITTEN);
    let mut file = ConfigFile::load(&path).unwrap();
    let mut cfg = file.config().clone();
    assert_eq!(cfg.general.theme, "dark");
    cfg.general.play_sound = false;
    file.save(&cfg).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("# my notes"));
    assert!(after.contains("theme = \"dark\" # keep"));
    assert!(after.contains("[mystery]\nvalue = 3"));
    assert!(after.contains("play_sound = false"));
    let reloaded = ConfigFile::load(&path).unwrap();
    assert_eq!(reloaded.config(), &cfg);
}

#[test]
fn unchanged_config_does_not_rewrite_hand_formatting() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(
        dir.path(),
        "[ocr]\nlanguages = [ \"eng\" ]\n[default_look]\npadding = 0.080\n",
    );
    let mut file = ConfigFile::load(&path).unwrap();
    let cfg = file.config().clone();
    file.save(&cfg).unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("languages = [ \"eng\" ]"));
    assert!(after.contains("padding = 0.080"));
}

#[test]
fn a_broken_config_reports_line_and_column_and_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let broken = "[general]\ntheme = \n";
    let path = write(dir.path(), broken);
    let mut file = ConfigFile::load(&path).unwrap();
    match file.error() {
        Some(Error::Parse { line, col, .. }) => assert_eq!((*line, *col), (2, 9)),
        other => panic!("{other:?}"),
    }
    let cfg = file.config().clone();
    assert!(matches!(file.save(&cfg), Err(Error::Damaged)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

#[test]
fn reload_after_a_bad_edit_keeps_the_last_good_config_then_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "[general]\ntheme = \"dark\"\n");
    let mut file = ConfigFile::load(&path).unwrap();
    std::fs::write(&path, "[general\n").unwrap();
    let err = file.reload().unwrap_err();
    assert!(matches!(err, Error::Parse { line: 1, .. }));
    assert_eq!(file.config().general.theme, "dark");
    let cfg = file.config().clone();
    assert!(file.save(&cfg).is_err());
    std::fs::write(&path, "[general]\ntheme = \"light\"\n").unwrap();
    assert!(file.reload().unwrap());
    assert_eq!(file.config().general.theme, "light");
    assert!(file.error().is_none());
}

#[test]
fn a_wrong_typed_value_is_a_parse_error_with_position() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "[general]\nplay_sound = \"yes\"\n");
    let file = ConfigFile::load(&path).unwrap();
    assert!(matches!(file.error(), Some(Error::Parse { line: 2, .. })));
}

#[test]
fn a_newer_config_version_is_never_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "config_version = 9\n");
    let mut file = ConfigFile::load(&path).unwrap();
    let cfg = file.config().clone();
    assert!(matches!(
        file.save(&cfg),
        Err(Error::NewerVersion { version: 9 })
    ));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "config_version = 9\n"
    );
}

#[test]
fn clearing_the_save_dir_removes_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "[general]\nsave_dir = \"/tmp/x\"\n");
    let mut file = ConfigFile::load(&path).unwrap();
    let mut cfg = file.config().clone();
    cfg.general.save_dir = None;
    file.save(&cfg).unwrap();
    assert!(!std::fs::read_to_string(&path).unwrap().contains("save_dir"));
}

#[test]
fn watcher_fires_once_per_external_edit_burst() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "[general]\ntheme = \"dark\"\n");
    let (tx, rx) = mpsc::channel();
    let _watcher = FileWatcher::new(&path, move || {
        let _ = tx.send(());
    })
    .unwrap();
    std::fs::write(dir.path().join("other.toml"), "x").unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "other files are ignored"
    );
    std::fs::write(&path, "[general]\ntheme = \"light\"\n").unwrap();
    std::fs::write(&path, "[general]\ntheme = \"system\"\n").unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "a burst is one event"
    );
}

#[test]
fn watcher_sees_an_atomic_replace() {
    let dir = tempfile::tempdir().unwrap();
    let path = write(dir.path(), "a = 1\n");
    let (tx, rx) = mpsc::channel();
    let _watcher = FileWatcher::new(&path, move || {
        let _ = tx.send(());
    })
    .unwrap();
    es_store::atomic::write_atomic(&path, b"a = 2\n").unwrap();
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
}
