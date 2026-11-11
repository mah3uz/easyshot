#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::gallery_support::shares::{SharedRig, record, shared_rig};
use es_app::share::ShareStart;
use es_model::{ShareKind, ShareRecord, ShareSource, ShareTarget};
use es_share::Media;
use es_shell::gallery::shared::label;
use es_shell::gallery::{Category, GalleryEvent, Item, Section, ShareAction};
use es_ui::tokens::ThemeMode;
use gpui::Entity;
use gpui_component::WindowExt;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

fn capture(id: Uuid) -> ShareSource {
    ShareSource::Capture(id)
}

/// Item 0 has a three-day free link, 1 one that ends in five hours, 2 a bucket link, 3 an expired one; 4 has none.
fn four_states(ids: &[Uuid]) -> Vec<ShareRecord> {
    let live = record("live", capture(ids[0]), Some(now() + Duration::days(3)));
    let soon = record("soon", capture(ids[1]), Some(now() + Duration::hours(5)));
    let mut bucket = record("bucket", capture(ids[2]), None);
    bucket.target = ShareTarget::Bucket {
        label: "team-screens".into(),
    };
    bucket.days = None;
    let mut dead = record("dead", capture(ids[3]), Some(now() - Duration::hours(1)));
    dead.created_at = now() - Duration::days(3);
    let mut records = vec![live, soon, bucket, dead];
    for (age, r) in records.iter_mut().enumerate().take(3) {
        r.created_at = now() - Duration::minutes(age as i64 + 1);
    }
    records
}

fn rig(mode: ThemeMode) -> (SharedRig, Vec<Uuid>) {
    let mut ids = Vec::new();
    let rig = shared_rig(8, mode, |_, entries| {
        ids = entries.iter().map(|e| e.id).collect();
        four_states(&ids)
    });
    (rig, ids)
}

fn item(rig: &mut SharedRig, id: Uuid) -> Item {
    rig.rig
        .read(|g| g.shown().iter().find(|i| i.id == id).cloned())
        .expect("listed")
}

fn act(rig: &mut SharedRig, id: Uuid, action: ShareAction) {
    let gallery: Entity<_> = rig.rig.gallery.clone();
    let handle = rig.rig.app.window.into();
    rig.rig
        .app
        .cx
        .update_window(handle, |_, window, cx| {
            gallery.update(cx, |g, cx| g.share_action(id, action, window, cx))
        })
        .unwrap();
    rig.rig.app.settle();
}

fn dialog_open(rig: &mut SharedRig) -> bool {
    let handle = rig.rig.app.window.into();
    rig.rig
        .app
        .cx
        .update_window(handle, |_, window, cx| window.has_active_dialog(cx))
        .unwrap()
}

#[test]
fn each_item_says_when_its_link_expires_where_it_went_or_that_it_expired() {
    let (mut rig, ids) = rig(ThemeMode::Dark);
    let text = |rig: &mut SharedRig, i: usize| {
        let record = item(rig, ids[i]).share.expect("shared");
        label(&record, now(), false)
    };
    assert_eq!(text(&mut rig, 0), "Expires in 3 days");
    assert_eq!(text(&mut rig, 1), "Expires in 5 hours");
    assert_eq!(text(&mut rig, 2), "team-screens");
    assert_eq!(text(&mut rig, 3), "Expired");
    assert!(item(&mut rig, ids[4]).share.is_none());
}

#[test]
fn copy_link_puts_the_page_on_the_clipboard_and_open_link_asks_the_app_to_open_it() {
    let (mut rig, ids) = rig(ThemeMode::Dark);
    act(&mut rig, ids[0], ShareAction::CopyLink);
    act(&mut rig, ids[0], ShareAction::CopyFileLink);
    assert_eq!(
        *rig.copied.0.borrow(),
        [
            "https://share.example/s/live",
            "https://share.example/m/live.png"
        ]
    );
    act(&mut rig, ids[0], ShareAction::OpenLink);
    assert_eq!(
        *rig.events.borrow(),
        [GalleryEvent::OpenUrl("https://share.example/s/live".into())]
    );
}

#[test]
fn delete_asks_first_then_deletes_the_link_and_keeps_the_file() {
    let (mut rig, ids) = rig(ThemeMode::Dark);
    let file = item(&mut rig, ids[0]).file;
    act(&mut rig, ids[0], ShareAction::Delete);
    assert!(
        dialog_open(&mut rig),
        "a link that works is never deleted unasked"
    );
    assert!(rig.service.deleted.lock().unwrap().is_empty());
    rig.rig.press("enter");
    let gallery = rig.rig.gallery.clone();
    let id = ids[0];
    rig.rig.app.until("the share is deleted", |cx| {
        gallery.read(cx).share_of(id).is_none()
    });
    assert_eq!(*rig.service.deleted.lock().unwrap(), ["live"]);
    assert!(file.is_file(), "deleting a link leaves the capture alone");
    assert_eq!(item(&mut rig, ids[0]).share, None);
}

#[test]
fn an_expired_link_offers_only_remove_and_remove_forgets_it_without_the_service() {
    let (mut rig, ids) = rig(ThemeMode::Dark);
    let offered = rig.rig.read(|g| g.share_actions(ids[3]));
    assert_eq!(offered, [ShareAction::Remove]);
    act(&mut rig, ids[3], ShareAction::CopyLink);
    act(&mut rig, ids[3], ShareAction::Delete);
    assert!(
        rig.copied.0.borrow().is_empty(),
        "a dead link is never copied"
    );
    assert!(!dialog_open(&mut rig));
    let live = rig.rig.read(|g| g.share_actions(ids[0]));
    assert!(!live.contains(&ShareAction::Remove));

    act(&mut rig, ids[3], ShareAction::Remove);
    assert_eq!(item(&mut rig, ids[3]).share, None);
    assert!(rig.service.deleted.lock().unwrap().is_empty());
    let cloud = rig.rig.read(|g| g.count_in(Section::Cloud, Category::All));
    assert_eq!(cloud, 3);
}

#[test]
fn cloud_lists_every_link_newest_first_even_when_its_file_is_gone() {
    let mut ids = Vec::new();
    let mut gone = PathBuf::new();
    let mut rig = shared_rig(8, ThemeMode::Dark, |_, entries| {
        ids = entries.iter().map(|e| e.id).collect();
        gone = entries[5].file.clone();
        std::fs::remove_file(&gone).unwrap();
        let mut records = four_states(&ids);
        let mut lost = record("lost", capture(ids[5]), Some(now() + Duration::days(1)));
        lost.title = Some("Login bug".into());
        lost.created_at = now() + Duration::minutes(1);
        let mut orphan = record("orphan", capture(Uuid::new_v4()), None);
        orphan.kind = ShareKind::Video;
        orphan.created_at = now() - Duration::days(9);
        records.extend([lost, orphan]);
        records
    });
    rig.rig
        .update(|g, cx| g.set_section(Section::Cloud, Category::All, cx));
    let shown = rig.rig.read(|g| g.shown().to_vec());
    let pages: Vec<_> = shown
        .iter()
        .map(|i| i.share.as_ref().unwrap().id.as_str())
        .collect();
    assert_eq!(pages, ["lost", "live", "soon", "bucket", "dead", "orphan"]);
    assert_eq!(shown[0].name, "Login bug");
    assert!(!shown[0].local && !shown[5].local && shown[1].local);
    assert!(!gone.exists());
    assert_eq!(
        rig.rig.read(|g| g.count(Category::All)),
        7,
        "the library lists only files that are there"
    );
    rig.rig
        .update(|g, cx| g.set_section(Section::Cloud, Category::Videos, cx));
    assert_eq!(rig.rig.read(|g| g.shown().len()), 1);
}

#[test]
fn the_gallery_follows_a_share_that_finishes_after_it_opened() {
    let mut ids = Vec::new();
    let mut file = PathBuf::new();
    let mut rig = shared_rig(4, ThemeMode::Dark, |_, entries| {
        ids = entries.iter().map(|e| e.id).collect();
        file = entries[1].file.clone();
        Vec::new()
    });
    assert!(item(&mut rig, ids[1]).share.is_none());
    let shares = rig.shares.clone();
    let start = ShareStart {
        media: Media {
            kind: ShareKind::Image,
            mime: "image/png".into(),
            path: file,
            width: 4,
            height: 3,
            duration_ms: None,
            poster: None,
        },
        title: None,
        days: 7,
        source: capture(ids[1]),
    };
    rig.rig.app.cx.update(|cx| {
        shares.update(cx, |s, cx| {
            s.consent(es_model::ShareMode::Free, cx);
            s.start(start, cx);
        })
    });
    let (gallery, id) = (rig.rig.gallery.clone(), ids[1]);
    rig.rig.app.until("the finished share is listed", |cx| {
        gallery.read(cx).share_of(id).is_some()
    });
    let record = item(&mut rig, ids[1]).share.unwrap();
    assert_eq!(label(&record, now(), false), "Expires in 7 days");
    assert_eq!(
        rig.rig.read(|g| g.count_in(Section::Cloud, Category::All)),
        1
    );
}

fn right_click(rig: &mut SharedRig, x: f32, y: f32) {
    use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point, px};
    let position = point(px(x), px(y));
    let handle = rig.rig.app.window.into();
    for event in [
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Right,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Right,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
    ] {
        rig.rig
            .app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .unwrap();
    }
    rig.rig.app.settle();
}

fn pictures(mode: ThemeMode, tag: &str) {
    let (mut rig, ids) = rig(mode);
    rig.rig.wait_for_thumbnails(8);
    rig.rig.update(|g, cx| g.select(Some(ids[0]), cx));
    rig.rig.picture(&format!("gallery-shares-grid-{tag}"));
    rig.rig.set_item_size(160);
    rig.rig.wait_for_sharp_thumbnails(4);
    rig.rig.picture(&format!("gallery-shares-grid-large-{tag}"));
    rig.rig.set_item_size(96);
    rig.rig.draw();
    right_click(&mut rig, 310., 134.);
    rig.rig.picture(&format!("gallery-shares-menu-{tag}"));
    rig.rig.press("escape");
    rig.rig
        .update(|g, cx| g.set_view(es_shell::gallery::ViewMode::List, cx));
    rig.rig.picture(&format!("gallery-shares-list-{tag}"));
    rig.rig
        .update(|g, cx| g.set_view(es_shell::gallery::ViewMode::Icons, cx));
    rig.rig
        .update(|g, cx| g.set_section(Section::Cloud, Category::All, cx));
    rig.rig.update(|g, cx| g.select(Some(g.shown()[3].id), cx));
    rig.rig.draw();
    right_click(&mut rig, 813., 134.);
    rig.rig.picture(&format!("gallery-shares-cloud-{tag}"));

    let mut empty = shared_rig(2, mode, |_, _| Vec::new());
    empty
        .rig
        .update(|g, cx| g.set_section(Section::Cloud, Category::All, cx));
    empty
        .rig
        .picture(&format!("gallery-shares-cloud-empty-{tag}"));
}

#[test]
fn pictures_shares_dark() {
    pictures(ThemeMode::Dark, "dark");
}

#[test]
fn pictures_shares_light() {
    pictures(ThemeMode::Light, "light");
}
