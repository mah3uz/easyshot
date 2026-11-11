use super::cells::kind_title;
use super::model::{Item, MediaKind, Section};
use super::view::{Gallery, GalleryEvent};
use crate::strings::fl;
use es_app::services::Strings;
use es_app::share::ShareDeleted;
use es_model::{HistoryEntry, ShareKind, ShareRecord, ShareSource, ShareTarget};
use es_ui::{
    a11y::{Accessible, Role},
    icons::IconName,
};
use gpui::{
    AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, Window, div, hsla, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, WindowExt, button::ButtonVariant};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareAction {
    CopyLink,
    OpenLink,
    CopyFileLink,
    Delete,
    Remove,
}

/// A dead link is never offered as if it worked: all that is left is to drop it from the list.
pub fn actions(record: &ShareRecord, now: OffsetDateTime) -> &'static [ShareAction] {
    if record.is_expired(now) {
        &[ShareAction::Remove]
    } else {
        &[
            ShareAction::CopyLink,
            ShareAction::OpenLink,
            ShareAction::CopyFileLink,
            ShareAction::Delete,
        ]
    }
}

/// A free link's time left, the bucket a link went to, or Expired; `short` fits on a preview.
pub fn label(record: &ShareRecord, now: OffsetDateTime, short: bool) -> String {
    if record.is_expired(now) {
        return fl!("gallery-share-expired");
    }
    match (&record.target, record.expires_at) {
        (ShareTarget::Bucket { label }, _) if !label.trim().is_empty() => label.clone(),
        (ShareTarget::Free, Some(at)) => {
            let hours = ((at - now).whole_seconds().max(1) as u64).div_ceil(3600);
            let days = (hours + 12) / 24;
            match (hours <= 36, short) {
                (true, false) => fl!("gallery-share-expires-hours", hours = hours),
                (true, true) => fl!("gallery-share-left-hours", hours = hours),
                (false, false) => fl!("gallery-share-expires-days", days = days),
                (false, true) => fl!("gallery-share-left-days", days = days),
            }
        }
        _ => fl!("gallery-share-live"),
    }
}

fn belongs(record: &ShareRecord, item: &Item) -> bool {
    match &record.source {
        ShareSource::Capture(id) => item.kind == MediaKind::Screenshot && item.id == *id,
        ShareSource::Recording(path) => {
            item.file == *path || item.project.as_ref().is_some_and(|p| p == path)
        }
    }
}

/// Each item gets its newest live link, else its newest expired one, so an expiry shows where it happened.
pub fn attach(items: &mut [Item], records: &[ShareRecord], now: OffsetDateTime) {
    for item in items {
        item.share = records
            .iter()
            .filter(|r| belongs(r, item))
            .max_by_key(|r| (!r.is_expired(now), r.created_at))
            .cloned();
    }
}

/// One item per link, with its local file when it still has one, so a link outlives its file.
pub fn cloud(records: &[ShareRecord], local: &[Item], history: &[HistoryEntry]) -> Vec<Item> {
    records
        .iter()
        .map(|record| {
            let found = local.iter().find(|i| belongs(record, i));
            let (file, name, duration_secs) = match (found, &record.source) {
                (Some(i), _) => (i.file.clone(), Some(i.name.clone()), i.duration_secs),
                (None, ShareSource::Capture(id)) => history
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| (e.file.clone(), Some(e.name.clone()), None))
                    .unwrap_or_default(),
                (None, ShareSource::Recording(path)) => (
                    path.clone(),
                    path.file_stem().map(|s| s.to_string_lossy().into_owned()),
                    None,
                ),
            };
            let kind = match record.kind {
                ShareKind::Image => MediaKind::Screenshot,
                ShareKind::Video => MediaKind::Video,
            };
            let name = record
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .or(name)
                .unwrap_or_else(|| kind_title(kind));
            Item {
                id: Uuid::new_v5(&Uuid::NAMESPACE_OID, record.id.as_bytes()),
                name,
                local: found.is_some(),
                file,
                created_at: record.created_at,
                kind,
                duration_secs,
                project: None,
                share: Some(record.clone()),
            }
        })
        .collect()
}

/// What an item's menu offers, read when the menu opens.
#[derive(Clone)]
pub struct Offer {
    pub kind: MediaKind,
    pub local: bool,
    pub cloud: bool,
    pub share: Vec<ShareAction>,
}

impl Gallery {
    pub fn share_of(&self, id: Uuid) -> Option<&ShareRecord> {
        self.item(id)?.share.as_ref()
    }

    /// Nothing while the link is being deleted.
    pub fn share_actions(&self, id: Uuid) -> Vec<ShareAction> {
        match self.share_of(id) {
            Some(r) if !self.deleting.contains(&r.id) => {
                actions(r, OffsetDateTime::now_utc()).to_vec()
            }
            _ => Vec::new(),
        }
    }

    pub fn is_deleting(&self, id: Uuid) -> bool {
        self.share_of(id)
            .is_some_and(|r| self.deleting.contains(&r.id))
    }

    pub(super) fn offer(&self, id: Uuid) -> Option<Offer> {
        let item = self.item(id)?;
        Some(Offer {
            kind: item.kind,
            local: item.local,
            cloud: self.section == Section::Cloud,
            share: self.share_actions(id),
        })
    }

    /// An action the item does not offer now, as from a menu left open across an expiry, does nothing.
    pub fn share_action(
        &mut self,
        id: Uuid,
        action: ShareAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.share_actions(id).contains(&action) {
            return;
        }
        let Some(record) = self.share_of(id).cloned() else {
            return;
        };
        match action {
            ShareAction::CopyLink => self.copy(record.page_url, cx),
            ShareAction::CopyFileLink => self.copy(record.media_url, cx),
            ShareAction::OpenLink => cx.emit(GalleryEvent::OpenUrl(record.page_url)),
            ShareAction::Delete => self.confirm_delete_share(id, window, cx),
            ShareAction::Remove => {
                if let Some(shares) = self.shares.clone() {
                    shares.update(cx, |s, cx| s.forget(&record.id, cx));
                }
            }
        }
    }

    fn copy(&self, text: String, cx: &mut Context<Self>) {
        if let Some(clipboard) = &self.clipboard {
            clipboard.copy(text, cx);
        }
    }

    /// Delete in the Cloud section: a live link asks first, an expired one goes at once.
    pub(super) fn delete_key(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let offered = self.share_actions(id);
        if offered.contains(&ShareAction::Delete) {
            self.share_action(id, ShareAction::Delete, window, cx);
        } else if offered.contains(&ShareAction::Remove) {
            self.share_action(id, ShareAction::Remove, window, cx);
        }
    }

    fn confirm_delete_share(&self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.item(id) else { return };
        let title = item.name.clone();
        let this = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let this = this.clone();
            dialog
                .title(fl!("gallery-delete-cloud-alert-title"))
                .description(fl!(
                    "gallery-delete-cloud-alert-message",
                    title = title.clone()
                ))
                .ok_text(fl!("gallery-delete-cloud-confirm"))
                .ok_variant(ButtonVariant::Danger)
                .cancel_text(fl!("gallery-cancel"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = this.update(cx, |this, cx| this.delete_share(id, cx));
                    true
                })
        });
    }

    /// Deletes the link where it is stored; the local file stays.
    pub fn delete_share(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let (Some(shares), Some(record)) = (self.shares.clone(), self.share_of(id).cloned()) else {
            return;
        };
        if !self.deleting.insert(record.id.clone()) {
            return;
        }
        shares.update(cx, |s, cx| s.delete(record, cx));
        cx.notify();
    }

    pub(super) fn on_deleted(&mut self, event: &ShareDeleted, cx: &mut Context<Self>) {
        if !self.deleting.remove(&event.record.id) {
            return;
        }
        if let Err(failure) = &event.result {
            self.error = Some(fl!(
                "gallery-error-delete-share",
                error = failure.message(&Strings)
            ));
        }
        cx.notify();
    }
}

/// The cloud mark on a preview, with the link's state beside it when there is room.
pub fn mark(
    id: Uuid,
    record: &ShareRecord,
    busy: bool,
    with_text: bool,
    now: OffsetDateTime,
) -> AnyElement {
    let expired = record.is_expired(now);
    let text = if busy {
        fl!("gallery-deleting")
    } else {
        label(record, now, true)
    };
    let icon = match (busy, expired) {
        (true, _) => IconName::Cloud,
        (false, true) => IconName::CloudOff,
        (false, false) => IconName::CloudCheck,
    };
    let ink: Hsla = if expired {
        hsla(0., 0., 1., 0.75)
    } else {
        hsla(0., 0., 1., 1.)
    };
    div()
        .id(SharedString::from(format!("share-{id}")))
        .accessible(
            Role::Image,
            fl!("gallery-share-a11y", state = label(record, now, false)),
        )
        .absolute()
        .left(px(2.))
        .top(px(2.))
        .max_w_full()
        .flex()
        .items_center()
        .gap(px(3.))
        .px(px(if with_text { 5. } else { 3. }))
        .py(px(if with_text { 1. } else { 3. }))
        .rounded_full()
        .bg(hsla(0., 0., 0., 0.7))
        .text_color(ink)
        .text_size(px(10.))
        .child(Icon::new(icon).size(px(11.)).flex_none())
        .when(with_text, |d| {
            d.child(div().min_w_0().truncate().child(text))
        })
        .into_any_element()
}

/// The link's state after an item's name in the list view.
pub fn list_mark(record: &ShareRecord, ink: Hsla, now: OffsetDateTime) -> impl IntoElement {
    let icon = if record.is_expired(now) {
        IconName::CloudOff
    } else {
        IconName::CloudCheck
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .pl(px(10.))
        .text_size(px(11.))
        .text_color(ink)
        .child(Icon::new(icon).size(px(12.)))
        .child(label(record, now, false))
}
