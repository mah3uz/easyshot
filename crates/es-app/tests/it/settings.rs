#![allow(clippy::unwrap_used)]
use es_app::settings_model::{SAVE_DEBOUNCE, SectionMask, SettingsEvent, SettingsModel};
use es_store::{ConfigFile, Paths};
use gpui::{AppContext, Entity, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

type Log = Rc<RefCell<Vec<SettingsEvent>>>;

fn config_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let path = paths.config_file();
    (dir, path)
}

fn model(cx: &mut TestAppContext, path: &std::path::Path) -> (Entity<SettingsModel>, Log) {
    let file = ConfigFile::load(path).unwrap();
    let model = cx.new(|_| SettingsModel::new(file));
    let log = Log::default();
    let sink = log.clone();
    cx.update(|cx| {
        cx.subscribe(&model, move |_, event: &SettingsEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (model, log)
}

fn toggle_sound(cx: &mut TestAppContext, model: &Entity<SettingsModel>) {
    model.update(cx, |m, cx| {
        m.update(|c| c.general.play_sound = !c.general.play_sound, cx)
    });
}

fn advance(cx: &mut TestAppContext, by: Duration) {
    cx.executor().advance_clock(by);
    cx.run_until_parked();
}

#[gpui::test]
fn update_emits_only_the_sections_that_changed(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, log) = model(cx, &path);
    model.update(cx, |m, cx| {
        m.update(
            |c| {
                c.general.play_sound = !c.general.play_sound;
                c.ocr.languages = vec!["deu".into()];
            },
            cx,
        )
    });
    assert_eq!(
        *log.borrow(),
        vec![SettingsEvent::Changed(
            SectionMask::GENERAL | SectionMask::OCR
        )]
    );
}

#[gpui::test]
fn an_update_that_changes_nothing_emits_nothing_and_never_writes(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, log) = model(cx, &path);
    model.update(cx, |m, cx| m.update(|_| {}, cx));
    advance(cx, SAVE_DEBOUNCE * 2);
    assert!(log.borrow().is_empty());
    assert!(!path.exists());
}

#[gpui::test]
fn save_waits_for_a_quiet_period_and_happens_once(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, _log) = model(cx, &path);
    toggle_sound(cx, &model);
    advance(cx, Duration::from_millis(200));
    assert!(!path.exists(), "written before the debounce elapsed");
    toggle_sound(cx, &model);
    advance(cx, Duration::from_millis(200));
    assert!(
        !path.exists(),
        "a second edit must restart the quiet period"
    );
    advance(cx, Duration::from_millis(100));
    assert!(path.exists());
    let saved = ConfigFile::load(&path).unwrap();
    assert_eq!(
        saved.config().general.play_sound,
        model.read_with(cx, |m, _| m.config().general.play_sound)
    );
}

#[gpui::test]
fn an_outside_edit_is_loaded_and_announced(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, log) = model(cx, &path);
    let sound = model.read_with(cx, |m, _| m.config().general.play_sound);
    std::fs::write(&path, format!("[general]\nplay_sound = {}\n", !sound)).unwrap();
    model.update(cx, |m, cx| m.reload(cx));
    assert_eq!(
        *log.borrow(),
        vec![SettingsEvent::Changed(SectionMask::GENERAL)]
    );
    assert_eq!(
        model.read_with(cx, |m, _| m.config().general.play_sound),
        !sound
    );
}

#[gpui::test]
fn the_models_own_write_does_not_come_back_as_an_edit(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, log) = model(cx, &path);
    toggle_sound(cx, &model);
    advance(cx, SAVE_DEBOUNCE);
    log.borrow_mut().clear();
    model.update(cx, |m, cx| m.reload(cx));
    assert!(log.borrow().is_empty());
}

#[gpui::test]
fn a_file_broken_at_launch_is_reported_and_never_overwritten(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let broken = "[general]\nplay_sound = maybe\n";
    std::fs::write(&path, broken).unwrap();
    let (model, _log) = model(cx, &path);
    assert!(matches!(
        model.read_with(cx, |m, _| m.load_error()),
        Some(es_app::errors::Error::Parse { line: 2, .. })
    ));
    toggle_sound(cx, &model);
    advance(cx, SAVE_DEBOUNCE * 2);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
}

#[gpui::test]
fn breaking_the_file_keeps_the_last_good_config_and_is_never_overwritten(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    let (model, log) = model(cx, &path);
    toggle_sound(cx, &model);
    advance(cx, SAVE_DEBOUNCE);
    let good = model.read_with(cx, |m, _| m.config().clone());
    log.borrow_mut().clear();

    let broken = "[general]\nplay_sound = maybe\n";
    std::fs::write(&path, broken).unwrap();
    model.update(cx, |m, cx| m.reload(cx));
    let events = log.borrow().clone();
    assert!(
        matches!(events.as_slice(), [SettingsEvent::Invalid { line: 2, col, .. }] if *col > 0),
        "{events:?}"
    );
    assert_eq!(model.read_with(cx, |m, _| m.config().clone()), good);

    model.update(cx, |m, cx| {
        m.update(|c| c.ocr.languages = vec!["fra".into()], cx)
    });
    advance(cx, SAVE_DEBOUNCE * 2);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);

    std::fs::write(&path, "[general]\nplay_sound = true\n").unwrap();
    log.borrow_mut().clear();
    model.update(cx, |m, cx| m.reload(cx));
    assert!(matches!(
        log.borrow().as_slice(),
        [SettingsEvent::Changed(_)]
    ));
}

fn unknown_events(log: &Log) -> Vec<Vec<String>> {
    log.borrow()
        .iter()
        .filter_map(|e| match e {
            SettingsEvent::UnknownKeys(keys) => Some(keys.iter().map(|k| k.path.clone()).collect()),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn unknown_keys_are_reported_once_per_changed_set(cx: &mut TestAppContext) {
    let (_dir, path) = config_path();
    std::fs::write(&path, "oops = 1\n").unwrap();
    let (model, log) = model(cx, &path);
    model.update(cx, |m, cx| m.report_unknown(cx));
    model.update(cx, |m, cx| m.report_unknown(cx));
    assert_eq!(unknown_events(&log), vec![vec!["oops".to_owned()]]);

    model.update(cx, |m, cx| m.reload(cx));
    assert_eq!(unknown_events(&log).len(), 1);

    std::fs::write(&path, "oops = 1\nalso = 2\n").unwrap();
    model.update(cx, |m, cx| m.reload(cx));
    assert_eq!(unknown_events(&log).len(), 2);
    assert_eq!(unknown_events(&log)[1], vec!["oops", "also"]);

    std::fs::write(&path, "[general]\nplay_sound = true\n").unwrap();
    model.update(cx, |m, cx| m.reload(cx));
    assert_eq!(unknown_events(&log).len(), 2);
    std::fs::write(&path, "oops = 1\n").unwrap();
    model.update(cx, |m, cx| m.reload(cx));
    assert_eq!(unknown_events(&log).len(), 3);
}
