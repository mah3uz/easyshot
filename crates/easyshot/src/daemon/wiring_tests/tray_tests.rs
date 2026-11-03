use super::{Rig, png, rig};
use gpui::TestAppContext;
use std::rc::Rc;
use std::sync::Arc;

struct TrayRig {
    tray: Arc<es_desktop::RecordingTray>,
    pinned: Rc<std::cell::Cell<bool>>,
    feed: Rc<crate::daemon::tray::TrayFeed>,
    _follow: Vec<gpui::Subscription>,
}

impl TrayRig {
    fn menu(&self) -> Vec<es_desktop::TrayItem> {
        self.tray.menus().pop().expect("a menu was shown").items
    }

    fn item(&self, id: &str) -> Option<es_desktop::TrayItem> {
        self.menu().into_iter().find(|i| i.label_id == id)
    }

    fn recent(&self, submenu: &str) -> Vec<String> {
        let recent = self.item("tray-recent").expect("the recent submenu");
        let list = recent.children.into_iter().find(|c| c.label_id == submenu);
        list.expect("a submenu")
            .children
            .into_iter()
            .filter(|i| !i.label_id.ends_with("-empty"))
            .filter_map(|i| i.label)
            .collect()
    }
}

fn tray_rig(rig: &Rig, cx: &mut TestAppContext) -> TrayRig {
    let tray = Arc::new(es_desktop::RecordingTray::default());
    let pinned = Rc::new(std::cell::Cell::new(false));
    cx.update(|cx| {
        let feed = crate::daemon::tray::TrayFeed::new(
            {
                let tray = tray.clone();
                Rc::new(move |input| es_shell::tray::show(tray.as_ref(), input))
            },
            {
                let pinned = pinned.clone();
                Rc::new(move || pinned.get())
            },
            rig.history.clone(),
            rig.settings.clone(),
            rig.paths.videos_dir(),
        );
        let follow = feed.follow(&rig.deck, cx);
        TrayRig {
            tray,
            pinned,
            feed,
            _follow: follow,
        }
    })
}

fn entry(rig: &Rig, name: &str) -> es_model::HistoryEntry {
    let file = rig.dir.path().join(format!("{name}.png"));
    png(&file);
    es_model::HistoryEntry {
        id: uuid::Uuid::new_v4(),
        created_at: time::OffsetDateTime::UNIX_EPOCH,
        kind: es_model::CaptureKind::Region,
        name: name.into(),
        file,
        export_path: None,
        pixel_size: (16, 12),
        has_annotations: false,
        share_url: None,
    }
}

#[gpui::test]
fn a_capture_that_lands_in_history_reaches_the_trays_recent_list(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.recent("tray-recent-screenshots").is_empty());
    let shot = entry(&rig, "Fresh capture");
    cx.update(|cx| {
        rig.history
            .update(cx, |h, cx| h.add(shot, cx))
            .expect("history accepts it")
    });
    assert_eq!(
        tray.recent("tray-recent-screenshots"),
        vec!["Fresh capture".to_owned()]
    );
}

#[gpui::test]
fn a_saved_global_shortcut_labels_its_tray_row(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    assert_eq!(tray.item("tray-region").unwrap().shortcut, None);
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(
                |c| {
                    c.global_shortcuts
                        .insert("capture-region".into(), "Super+Shift+4".into());
                },
                cx,
            )
        })
    });
    assert_eq!(
        tray.item("tray-region").unwrap().shortcut.as_deref(),
        Some("Super+Shift+4")
    );
}

#[gpui::test]
fn unpin_all_shows_only_while_something_is_pinned(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.item("tray-unpin-all").is_none());
    tray.pinned.set(true);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.item("tray-unpin-all").is_some());
}

#[gpui::test]
fn saved_videos_fill_the_trays_recent_recordings(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, cx);
    std::fs::create_dir_all(rig.paths.videos_dir()).unwrap();
    std::fs::write(rig.paths.videos_dir().join("clip.mp4"), b"v").unwrap();
    std::fs::write(rig.paths.videos_dir().join("notes.txt"), b"t").unwrap();
    cx.update(|cx| tray.feed.refresh(cx));
    assert_eq!(
        tray.recent("tray-recent-recordings"),
        vec!["clip.mp4".to_owned()]
    );
}
