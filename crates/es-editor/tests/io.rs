#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod shell;

use es_editor::session::journal::{self, DELAY};
use es_editor::window::FLASH;
use es_editor::{EditorSession, EditorSource, Job, Progress};
use es_model::platform::ClipItem;
use shell::launch;
use std::time::Duration;

#[test]
fn save_writes_the_library_copy_sidecar_and_export_and_shows_saved() {
    let mut f = launch();
    f.edit();
    assert!(f.dirty());
    f.request(Job::Save);

    let (state, error) = f.editor.read_with(&f.app.cx, |w, _| {
        (w.progress(Job::Save), w.error().cloned())
    });
    assert_eq!((state, error), (Progress::Done, None));

    let library = f.paths.library_dir().join("capture.png");
    assert!(library.exists(), "flattened image in the library");
    let reopened = es_store::reopen(&library);
    assert_eq!(reopened.document.as_ref().map(|d| d.shapes.len()), Some(2));
    assert!(reopened.source.exists(), "the untouched base is kept");
    let exports: Vec<_> = std::fs::read_dir(&f.shots).unwrap().collect();
    assert_eq!(exports.len(), 1, "one file in the save folder");
    assert!(!f.dirty(), "saved edits are no longer unsaved");

    f.wait(FLASH);
    assert_eq!(
        f.editor.read_with(&f.app.cx, |w, _| w.progress(Job::Save)),
        Progress::Idle
    );
}

#[test]
fn saving_again_replaces_the_export_instead_of_adding_one() {
    let mut f = launch();
    f.request(Job::Save);
    f.wait(FLASH);
    f.edit();
    f.request(Job::Save);
    assert_eq!(std::fs::read_dir(&f.shots).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(f.paths.library_dir()).unwrap().count(), 3);
}

#[test]
fn export_writes_to_the_path_the_chooser_returns() {
    let mut f = launch();
    let target = f.dir.path().join("picked.png");
    *f.chooser.answer.borrow_mut() = Some(target.clone());
    f.request(Job::Export);

    let bytes = std::fs::read(&target).unwrap();
    assert_eq!(&bytes[1..4], b"PNG");
    let asked = f.chooser.asked.borrow();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0, "Export Screenshot");
    assert_eq!(asked[0].1, f.shots);
    assert_eq!(asked[0].2, "capture.png");
    assert_eq!(
        f.editor
            .read_with(&f.app.cx, |w, _| w.progress(Job::Export)),
        Progress::Idle
    );
}

#[test]
fn export_takes_its_format_from_the_chosen_extension() {
    let mut f = launch();
    let target = f.dir.path().join("picked.jpg");
    *f.chooser.answer.borrow_mut() = Some(target.clone());
    f.request(Job::Export);
    assert_eq!(&std::fs::read(&target).unwrap()[..2], &[0xFF, 0xD8]);
}

#[test]
fn dismissing_the_chooser_exports_nothing_and_is_not_an_error() {
    let mut f = launch();
    f.request(Job::Export);
    let error = f.editor.read_with(&f.app.cx, |w, _| w.error().cloned());
    assert_eq!(error, None);
    assert!(!f.dir.path().join("picked.png").exists());
}

#[test]
fn copy_hands_png_bytes_to_the_clipboard_sink() {
    let mut f = launch();
    f.request(Job::Copy);
    let items = f.sink.items.borrow();
    assert_eq!(items.len(), 1);
    let [ClipItem::Png(png)] = items[0].as_slice() else {
        panic!("copy must offer exactly the PNG; the sink adds the file entries");
    };
    let decoded = image::load_from_memory(png).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (1200, 640));
}

#[test]
fn a_failed_copy_shows_the_error_state() {
    let mut f = launch();
    *f.sink.fail.borrow_mut() = true;
    f.request(Job::Copy);
    let (state, error) = f.editor.read_with(&f.app.cx, |w, _| {
        (w.progress(Job::Copy), w.error().cloned())
    });
    assert_eq!(state, Progress::Idle);
    let error = error.expect("the failure is shown");
    assert!(error.starts_with("Failed to copy annotation:"), "{error}");
}

#[test]
fn a_failed_save_shows_the_error_and_stays_unsaved() {
    let mut f = launch();
    let blocker = f.dir.path().join("blocker");
    std::fs::write(&blocker, b"a file, not a folder").unwrap();
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
    f.edit();
    f.request(Job::Save);
    let error = f.editor.read_with(&f.app.cx, |w, _| w.error().cloned());
    assert!(error.unwrap().starts_with("Failed to save annotation:"));
    assert!(f.dirty(), "a failed save must not clear the unsaved state");
}

#[test]
fn the_journal_is_written_once_after_a_burst_of_edits() {
    let mut f = launch();
    for _ in 0..4 {
        f.edit();
        f.wait(Duration::from_millis(900));
        assert!(
            f.journal_files().is_empty(),
            "written before the burst ended"
        );
    }
    f.wait(DELAY - Duration::from_millis(900));
    let files = f.journal_files();
    assert_eq!(files.len(), 1);

    std::fs::remove_file(&files[0]).unwrap();
    f.wait(DELAY * 3);
    assert!(
        f.journal_files().is_empty(),
        "no second write for one burst"
    );
}

#[test]
fn the_journal_survives_a_crash_and_reopens_the_same_document() {
    let mut f = launch();
    f.edit();
    f.edit();
    f.wait(DELAY);
    let expected = f
        .session_doc()
        .read_with(&f.app.cx, |d, _| d.editor().doc().model().clone());

    let found = journal::recover(&f.paths);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].entry.document, expected);
    assert_eq!(found[0].entry.display, f.dir.path().join("capture.png"));
    let leftovers: Vec<_> = std::fs::read_dir(f.paths.recovery_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers.len(),
        1,
        "an atomic write leaves no temp file: {leftovers:?}"
    );

    let task = f
        .app
        .cx
        .update(|cx| EditorSession::open(EditorSource::Recovery(found[0].path.clone()), cx));
    let reopened = f.app.cx.foreground_executor().block_test(task).unwrap();
    f.app.settle();
    let (doc, dirty) = f.app.cx.update(|cx| {
        (
            reopened.doc.read(cx).editor().doc().model().clone(),
            reopened.io.read(cx).is_dirty(cx),
        )
    });
    assert_eq!(doc, expected);
    assert!(dirty, "recovered edits count as unsaved");
}

#[test]
fn an_unreadable_journal_is_skipped_and_left_alone() {
    let f = launch();
    let bad = f.paths.recovery_dir().join("broken.json");
    std::fs::write(&bad, b"{ not json").unwrap();
    assert!(journal::recover(&f.paths).is_empty());
    assert!(bad.exists());
}

#[test]
fn saving_deletes_the_journal() {
    let mut f = launch();
    f.edit();
    f.wait(DELAY);
    assert_eq!(f.journal_files().len(), 1);
    f.request(Job::Save);
    assert!(f.journal_files().is_empty());
    f.wait(DELAY * 2);
    assert!(
        f.journal_files().is_empty(),
        "a late timer must not bring it back"
    );
}

#[test]
fn undoing_back_to_the_saved_state_deletes_the_journal() {
    let mut f = launch();
    f.edit();
    f.wait(DELAY);
    assert_eq!(f.journal_files().len(), 1);
    f.editor.update(&mut f.app.cx, |w, cx| w.undo(cx));
    f.wait(DELAY);
    assert!(!f.dirty());
    assert!(f.journal_files().is_empty());
}

fn export_with_wallpaper(
    f: &mut shell::Fixture,
    wallpaper: &std::path::Path,
    name: &str,
) -> ([u8; 4], es_model::BackgroundSettings) {
    let look = es_model::BackgroundSettings {
        style: es_model::BackgroundStyle::Wallpaper {
            path: wallpaper.to_path_buf(),
        },
        ..es_model::BackgroundSettings::default()
    };
    let model = f
        .editor
        .read_with(&f.app.cx, |w, _| w.session().unwrap().look.clone());
    f.app
        .cx
        .update(|cx| model.update(cx, |l, cx| l.set(look.clone(), cx)));
    let target = f.dir.path().join(name);
    *f.chooser.answer.borrow_mut() = Some(target.clone());
    f.request(Job::Export);
    let exported = image::open(&target).unwrap().into_rgba8();
    (exported.get_pixel(2, 2).0, look)
}

#[test]
fn an_export_draws_its_wallpaper_like_the_canvas_does() {
    use es_editor::canvas::underlay::{UnderlayKey, render};

    let mut f = launch();
    let red = f.dir.path().join("red.png");
    image::RgbaImage::from_pixel(64, 64, image::Rgba([230, 20, 20, 255]))
        .save(&red)
        .unwrap();

    let (with, look) = export_with_wallpaper(&mut f, &red, "with.png");
    assert_eq!(with, [230, 20, 20, 255], "the wallpaper fills the margin");
    let key = UnderlayKey::new(&look, (1200, 640), 1300.0, 1.0).unwrap();
    let c = render(&key).unwrap().pixel(2, 2).unwrap();
    assert_eq!(
        [c.red(), c.green(), c.blue(), c.alpha()],
        with,
        "export and canvas draw the same wallpaper"
    );

    let gone = f.dir.path().join("gone.png");
    let (without, _) = export_with_wallpaper(&mut f, &gone, "without.png");
    assert_ne!(without, with, "a missing wallpaper is not drawn");
}
