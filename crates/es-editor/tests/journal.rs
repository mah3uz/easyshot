#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;
mod shell;

use es_anno::crop::{CropHandle, ImageOp};
use es_editor::session::crop::{CropModel, CropTarget};
use es_editor::session::io::{Choice, IoConfig};
use es_editor::session::journal::{self, DELAY, Entry, Journal};
use es_editor::{EditorSession, EditorSource, Job};
use es_geom::Point;
use es_model::AnnoShape;
use gpui::{Context, Entity};
use image::RgbaImage;
use shell::{Fixture, launch};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn crop_model(f: &Fixture) -> Entity<CropModel> {
    f.editor
        .read_with(&f.app.cx, |w, _| w.session().unwrap().crop.clone())
}

fn with_crop<R>(f: &mut Fixture, op: impl FnOnce(&mut CropModel, &mut Context<CropModel>) -> R) {
    let crop = crop_model(f);
    f.app.cx.update(|cx| crop.update(cx, op));
    f.app.settle();
}

fn rotate(f: &mut Fixture) {
    with_crop(f, |c, cx| assert!(c.transform(ImageOp::RotateLeft, cx)));
}

/// Trims 50 px off the left and 40 px off the top.
fn crop_top_left(f: &mut Fixture) {
    with_crop(f, |c, cx| {
        c.begin(cx);
        c.press(CropTarget::Handle(CropHandle::TopLeft), Point::ZERO);
        c.drag_to(Point::new(50.0, 40.0), false, cx);
        c.release(cx);
        assert!(c.apply(cx));
    });
}

fn live(f: &Fixture) -> (RgbaImage, Vec<AnnoShape>) {
    f.session_doc().read_with(&f.app.cx, |d, _| {
        ((**d.base()).clone(), d.editor().doc().snapshot().shapes)
    })
}

fn the_journal(f: &Fixture) -> (PathBuf, Entry) {
    let mut found = journal::recover(&f.paths);
    assert_eq!(found.len(), 1, "one journal");
    let r = found.remove(0);
    (r.path, r.entry)
}

/// Every PNG in the recovery directory, whatever its name.
fn staged(f: &Fixture) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(f.paths.recovery_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    out.sort();
    out
}

fn reopen(f: &mut Fixture, journal: PathBuf) -> EditorSession {
    let task = f
        .app
        .cx
        .update(|cx| EditorSession::open(EditorSource::Recovery(journal), cx));
    let session = f.app.cx.foreground_executor().block_test(task).unwrap();
    f.app.settle();
    session
}

fn reopened(f: &Fixture, s: &EditorSession) -> (RgbaImage, Vec<AnnoShape>) {
    s.doc.read_with(&f.app.cx, |d, _| {
        ((**d.base()).clone(), d.editor().doc().snapshot().shapes)
    })
}

#[test]
fn a_crop_survives_a_crash_with_its_pixels_and_its_annotations_in_place() {
    let mut f = launch();
    crop_top_left(&mut f);
    f.wait(DELAY);
    let (base, shapes) = live(&f);
    assert_eq!(base.dimensions(), (1150, 600));
    assert_eq!((shapes[0].x, shapes[0].y), (50.0, 60.0));

    let (path, entry) = the_journal(&f);
    let file = entry
        .staged_base
        .clone()
        .expect("the cropped base is staged");
    assert_eq!(staged(&f), vec![file.clone()]);
    assert_eq!(entry.base_image, f.dir.path().join("capture.png"));

    let s = reopen(&mut f, path);
    let (got_base, got_shapes) = reopened(&f, &s);
    assert_eq!(got_base.dimensions(), (1150, 600), "the crop is kept");
    assert!(got_base == base, "the staged pixels are the cropped ones");
    assert_eq!(got_shapes, shapes, "annotations sit where they were drawn");
    assert!(s.io.read_with(&f.app.cx, |io, cx| io.is_dirty(cx)));

    let paths = f.paths.clone();
    f.app.cx.update(|cx| {
        s.io.update(cx, |io, cx| {
            io.configure(IoConfig {
                paths: Some(paths),
                ..IoConfig::default()
            });
            io.flush_journal(cx);
        })
    });
    f.app.settle();
    assert_eq!(
        staged(&f),
        vec![file],
        "the recovered session keeps the staged base instead of encoding it again"
    );
}

#[test]
fn undoing_the_last_image_step_points_the_journal_back_at_the_original() {
    let mut f = launch();
    f.edit();
    rotate(&mut f);
    f.wait(DELAY);
    assert!(the_journal(&f).1.staged_base.is_some());
    assert_eq!(staged(&f).len(), 1);

    f.editor.update(&mut f.app.cx, |w, cx| w.undo(cx));
    f.wait(DELAY);
    assert!(f.dirty(), "the nudge is still unsaved");
    let (_, entry) = the_journal(&f);
    assert_eq!(entry.staged_base, None);
    assert_eq!(entry.base_image, f.dir.path().join("capture.png"));
    assert!(staged(&f).is_empty(), "the staged base is deleted");

    f.editor.update(&mut f.app.cx, |w, cx| w.redo(cx));
    f.wait(DELAY);
    let (_, entry) = the_journal(&f);
    assert_eq!(staged(&f), vec![entry.staged_base.unwrap()]);
}

#[test]
fn rotating_twice_leaves_one_staged_base() {
    let mut f = launch();
    rotate(&mut f);
    f.wait(DELAY);
    let first = staged(&f);
    assert_eq!(first.len(), 1);
    rotate(&mut f);
    f.wait(DELAY);
    let (path, entry) = the_journal(&f);
    let second = staged(&f);
    assert_eq!(second.len(), 1, "the older staged base is deleted");
    assert_ne!(first, second);
    assert_eq!(entry.staged_base.as_ref(), second.first());

    let (base, _) = live(&f);
    let s = reopen(&mut f, path);
    assert!(
        reopened(&f, &s).0 == base,
        "recovery opens the newest pixels"
    );
}

#[test]
fn a_missing_or_unreadable_staged_base_opens_the_original() {
    for damage in [
        |p: &PathBuf| std::fs::remove_file(p).unwrap(),
        |p: &PathBuf| std::fs::write(p, b"not a png").unwrap(),
    ] {
        let mut f = launch();
        let original = live(&f).0;
        rotate(&mut f);
        f.wait(DELAY);
        let (path, entry) = the_journal(&f);
        damage(&entry.staged_base.unwrap());

        let s = reopen(&mut f, path);
        let base = reopened(&f, &s).0;
        assert_eq!(base.dimensions(), (1200, 640));
        assert!(base == original);
    }
}

#[test]
fn saving_or_discarding_deletes_the_journal_and_its_staged_base() {
    for save in [true, false] {
        let mut f = launch();
        rotate(&mut f);
        f.wait(DELAY);
        assert_eq!(
            std::fs::read_dir(f.paths.recovery_dir()).unwrap().count(),
            2
        );
        if save {
            f.request(Job::Save);
        } else {
            f.should_close();
            let io = f.io();
            f.app
                .cx
                .update(|cx| io.update(cx, |io, cx| io.choose(Choice::Discard, cx)));
            f.app.settle();
        }
        f.wait(DELAY * 2);
        let left: Vec<_> = std::fs::read_dir(f.paths.recovery_dir())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(left.is_empty(), "save {save}: {left:?}");
    }
}

#[test]
fn a_burst_of_image_edits_stages_the_base_once() {
    let mut f = launch();
    for _ in 0..4 {
        rotate(&mut f);
        f.wait(Duration::from_millis(900));
        assert!(staged(&f).is_empty(), "staged before the burst ended");
    }
    f.wait(DELAY - Duration::from_millis(900));
    let once = staged(&f);
    assert_eq!(once.len(), 1);

    f.edit();
    f.wait(DELAY);
    let (_, entry) = the_journal(&f);
    assert_eq!(
        entry.document.shapes,
        live(&f).1,
        "the journal follows the edit"
    );
    assert_eq!(
        staged(&f),
        once,
        "an annotation edit does not encode the base again"
    );
}

#[test]
fn a_staged_write_overtaken_by_a_newer_write_or_a_removal_never_lands() {
    let dir = tempfile::tempdir().unwrap();
    let paths = es_store::Paths::rooted(dir.path());
    let pixels = Arc::new(canvas_support::base(64, 32));
    let entry = |base_revision| Entry {
        base_revision,
        ..Entry::default()
    };
    let j = Journal::default();

    let stale = j
        .write(&paths, "s", entry(2), Some(pixels.clone()))
        .unwrap();
    assert!(j.write(&paths, "s", entry(1), None).is_none());
    stale.run();
    assert_eq!(journal::recover(&paths).len(), 1);
    let kept = journal::load(&journal::journal_path(&paths, "s")).unwrap();
    assert_eq!(
        kept.map(|e| e.base_revision),
        Some(1),
        "an older write does not replace a newer journal"
    );
    assert!(journal::staged_files(&paths, "s").is_empty());

    let late = j.write(&paths, "s", entry(3), Some(pixels)).unwrap();
    j.remove(&paths, "s");
    late.run();
    assert!(
        journal::recover(&paths).is_empty(),
        "a removed journal stays removed"
    );
    assert!(journal::staged_files(&paths, "s").is_empty());
}
