#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::shell;

use es_editor::session::io::Choice;
use es_editor::session::journal::DELAY;
use shell::{Fixture, launch};

fn choose(f: &mut Fixture, choice: Choice) {
    let io = f.io();
    f.app
        .cx
        .update(|cx| io.update(cx, |io, cx| io.choose(choice, cx)));
    f.app.settle();
}

fn prompt_open(f: &Fixture) -> bool {
    f.io().read_with(&f.app.cx, |io, _| io.prompt_open())
}

#[test]
fn a_clean_editor_closes_without_asking() {
    let mut f = launch();
    assert!(f.should_close());
    assert!(!prompt_open(&f));
}

#[test]
fn a_dirty_editor_asks_and_cancel_keeps_it_open() {
    let mut f = launch();
    f.edit();
    assert!(!f.should_close(), "the close waits for the answer");
    assert!(prompt_open(&f));
    choose(&mut f, Choice::Cancel);
    assert!(f.window_open());
    assert!(f.dirty());
    assert!(!prompt_open(&f));
    f.app.screenshot("j12d2-close-guard-closed");
}

#[test]
fn the_prompt_shows_the_three_choices_and_the_file_name() {
    let mut f = launch();
    f.edit();
    f.should_close();
    f.app.screenshot("j12d2-close-guard");
}

#[test]
fn escape_cancels_and_other_shortcuts_are_suppressed_meanwhile() {
    let mut f = launch();
    f.edit();
    f.should_close();
    f.key("r");
    f.key("ctrl-s");
    assert!(
        f.events.borrow().is_empty(),
        "shortcuts are off while the dialog is up"
    );
    f.key("escape");
    assert!(!prompt_open(&f));
    assert!(f.window_open());
    assert!(f.dirty());
}

#[test]
fn ctrl_period_cancels_too() {
    let mut f = launch();
    f.edit();
    f.should_close();
    f.key("ctrl-.");
    assert!(!prompt_open(&f));
    assert!(f.window_open());
}

#[test]
fn discard_closes_and_removes_the_journal_without_saving() {
    let mut f = launch();
    f.edit();
    f.wait(DELAY);
    assert_eq!(f.journal_files().len(), 1);
    f.should_close();
    choose(&mut f, Choice::Discard);
    assert!(!f.window_open());
    assert!(f.journal_files().is_empty());
    assert!(!f.paths.library_dir().join("capture.png").exists());
}

#[test]
fn save_and_close_saves_then_closes() {
    let mut f = launch();
    f.edit();
    f.should_close();
    choose(&mut f, Choice::Save);
    f.app.settle();
    assert!(!f.window_open());
    assert!(f.paths.library_dir().join("capture.png").exists());
    assert_eq!(std::fs::read_dir(&f.shots).unwrap().count(), 1);
}

#[test]
fn a_failed_save_leaves_the_editor_open() {
    let mut f = launch();
    f.edit();
    let blocker = f.dir.path().join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    f.io().update(&mut f.app.cx, |io, _| {
        io.configure(es_editor::session::io::IoConfig {
            paths: Some(f.paths.clone()),
            settings: Some(es_editor::session::io::IoSettings {
                save_dir: blocker.join("inside"),
                template: "x".into(),
                format: es_store::ExportFormat::Png,
                quality: 0.9,
            }),
            ..Default::default()
        })
    });
    f.should_close();
    choose(&mut f, Choice::Save);
    assert!(f.window_open());
    assert!(f.dirty());
    let error = f.editor.read_with(&f.app.cx, |w, _| w.error().cloned());
    assert!(error.is_some());
}

#[test]
fn a_close_during_a_save_waits_for_it_and_closes_when_it_succeeds() {
    let mut f = launch();
    f.edit();
    f.editor.update(&mut f.app.cx, |w, cx| {
        w.request(es_editor::Job::Save, cx);
    });
    assert!(!f.should_close());
    f.app.settle();
    assert!(!f.window_open());
    assert!(!prompt_open(&f));
}
