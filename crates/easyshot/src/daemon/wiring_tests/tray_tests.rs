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

fn tray_rig(rig: &Rig, missing: bool, cx: &mut TestAppContext) -> TrayRig {
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
            rig.paths.clone(),
            missing,
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
    let tray = tray_rig(&rig, false, cx);
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
    let tray = tray_rig(&rig, false, cx);
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
    let tray = tray_rig(&rig, false, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.item("tray-unpin-all").is_none());
    tray.pinned.set(true);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.item("tray-unpin-all").is_some());
}

#[gpui::test]
fn saved_videos_fill_the_trays_recent_recordings(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, false, cx);
    std::fs::create_dir_all(rig.paths.videos_dir()).unwrap();
    std::fs::write(rig.paths.videos_dir().join("clip.mp4"), b"v").unwrap();
    std::fs::write(rig.paths.videos_dir().join("notes.txt"), b"t").unwrap();
    cx.update(|cx| tray.feed.refresh(cx));
    assert_eq!(
        tray.recent("tray-recent-recordings"),
        vec!["clip.mp4".to_owned()]
    );
}

#[gpui::test]
fn a_video_saved_or_recorded_after_the_menu_was_built_reaches_it_on_the_library_event(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, false, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    assert!(tray.recent("tray-recent-recordings").is_empty());

    std::fs::create_dir_all(rig.paths.videos_dir()).unwrap();
    std::fs::write(rig.paths.videos_dir().join("saved.mp4"), b"v").unwrap();
    let project = rig.paths.recordings_dir().join("unsaved.easyshotrec");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(project.join("screen.mkv"), b"v").unwrap();
    std::fs::write(project.join("capture.json"), r#"{"duration_secs":3.0}"#).unwrap();
    cx.update(|cx| rig.history.update(cx, |h, cx| h.videos_changed(cx)));

    let mut shown = tray.recent("tray-recent-recordings");
    shown.sort();
    assert_eq!(shown, vec!["saved.mp4".to_owned(), "unsaved".to_owned()]);
}

#[gpui::test]
fn without_a_recorder_the_tray_record_row_is_disabled_and_names_the_package(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let tray = tray_rig(&rig, true, cx);
    cx.update(|cx| tray.feed.refresh(cx));
    let record = tray.item("tray-record").expect("the record row stays");
    assert!(!record.enabled);
    assert_eq!(record.route, None);
    assert!(record.label.unwrap().contains("wf-recorder"));
    assert!(
        tray.item("tray-ocr").unwrap().enabled,
        "OCR still reads QR codes"
    );
}

#[gpui::test]
fn an_agent_grant_badges_the_tray_and_its_revoke_row_ends_it(cx: &mut TestAppContext) {
    use es_app::agent_access::{AgentAccess, AgentAccessHandle};
    use es_protocol::agent::GrantSpan;
    use gpui::AppContext as _;
    let rig = rig(cx);
    let tray = tray_rig(&rig, false, cx);
    let file = es_store::agent::open(&rig.paths);
    let (access, _follow) = cx.update(|cx| {
        let access = cx.new(|_| AgentAccess::new(file, es_app::agent_access::system_clock()));
        cx.set_global(AgentAccessHandle(access.clone()));
        let follow = tray.feed.follow_access_of(&access, cx);
        tray.feed.refresh(cx);
        (access, follow)
    });
    assert!(tray.item("tray-agent-revoke").is_none());
    assert_eq!(tray.tray.agent_states().last(), Some(&false));

    cx.update(|cx| access.update(cx, |a, cx| a.allow(GrantSpan::Session, cx)));
    let row = tray.item("tray-agent-revoke").expect("a revoke row");
    assert_eq!(row.route, Some(es_protocol::Route::AgentRevoke));
    assert_eq!(tray.tray.agent_states().last(), Some(&true));

    cx.update(crate::daemon::routes::revoke_agent);
    assert!(tray.item("tray-agent-revoke").is_none());
    assert_eq!(tray.tray.agent_states().last(), Some(&false));
}
