#![allow(clippy::unwrap_used)]
use es_app::history::{History, HistoryEvent};
use es_model::{CaptureKind, HistoryEntry};
use es_store::Paths;
use gpui::{AppContext, Entity, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use time::OffsetDateTime;
use uuid::Uuid;

type Log = Rc<RefCell<Vec<HistoryEvent>>>;

fn entry(name: &str, age_secs: i64) -> HistoryEntry {
    HistoryEntry {
        id: Uuid::new_v4(),
        created_at: OffsetDateTime::now_utc() - time::Duration::seconds(age_secs),
        kind: CaptureKind::Region,
        name: name.to_owned(),
        file: PathBuf::from(format!("/elsewhere/{name}.png")),
        export_path: None,
        pixel_size: (8, 6),
        has_annotations: false,
        share_url: Some("https://example.invalid/x".into()),
    }
}

fn history(cx: &mut TestAppContext, limit: usize) -> (tempfile::TempDir, Entity<History>, Log) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let model = cx.new(|_| History::open(&paths, limit).unwrap());
    let log = Log::default();
    let sink = log.clone();
    cx.update(|cx| {
        cx.subscribe(&model, move |_, event: &HistoryEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (dir, model, log)
}

#[gpui::test]
fn adding_announces_the_entry(cx: &mut TestAppContext) {
    let (_dir, history, log) = history(cx, 10);
    let e = entry("one", 0);
    let id = e.id;
    history.update(cx, |h, cx| h.add(e, cx).unwrap());
    assert_eq!(*log.borrow(), vec![HistoryEvent::Added(id)]);
    assert_eq!(history.read_with(cx, |h, _| h.entries().len()), 1);
}

#[gpui::test]
fn retention_announces_the_entries_it_drops(cx: &mut TestAppContext) {
    let (_dir, history, log) = history(cx, 2);
    let old = entry("old", 300);
    let old_id = old.id;
    history.update(cx, |h, cx| {
        h.add(old, cx).unwrap();
        h.add(entry("mid", 200), cx).unwrap();
        h.add(entry("new", 100), cx).unwrap();
    });
    assert!(log.borrow().contains(&HistoryEvent::Removed(vec![old_id])));
    assert!(history.read_with(cx, |h, _| h.get(old_id).is_none()));
}

#[gpui::test]
fn remove_and_rename_announce_only_when_something_changed(cx: &mut TestAppContext) {
    let (_dir, history, log) = history(cx, 10);
    let e = entry("one", 0);
    let id = e.id;
    history.update(cx, |h, cx| h.add(e, cx).unwrap());
    log.borrow_mut().clear();

    history.update(cx, |h, cx| {
        assert!(!h.rename(Uuid::new_v4(), "nothing", cx).unwrap());
        assert!(!h.remove(Uuid::new_v4(), cx).unwrap());
    });
    assert!(log.borrow().is_empty());

    history.update(cx, |h, cx| {
        assert!(h.rename(id, "renamed", cx).unwrap());
        assert!(h.remove(id, cx).unwrap());
    });
    assert_eq!(
        *log.borrow(),
        vec![HistoryEvent::Renamed(id), HistoryEvent::Removed(vec![id])]
    );
}
