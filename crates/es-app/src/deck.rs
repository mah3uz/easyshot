use es_model::config::{CaptureCfg, Overlay};
use es_model::{CaptureKind, DeckManifest};
use gpui::{Context, EventEmitter};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

pub const MAX_ITEMS: usize = 5;
pub const NEVER_DISMISS_SECS: u32 = 16;
const POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    Idle,
    Saving,
    SaveFailed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeckItem {
    pub id: Uuid,
    pub name: String,
    pub file: PathBuf,
    pub thumb: PathBuf,
    pub kind: CaptureKind,
    pub video: bool,
    /// Still only in the deck's staging directory, not in the library.
    pub staged: bool,
    pub status: ItemStatus,
    /// Items reopened from history or the tray stay until closed.
    pub auto_dismiss: bool,
    /// Where auto-save wrote the capture; the card offers Show in folder in place of Save.
    pub saved: Option<PathBuf>,
}

impl DeckItem {
    pub fn staged(m: &DeckManifest) -> Self {
        DeckItem {
            id: m.id,
            name: m.name.clone(),
            file: m.staged.clone(),
            thumb: m.thumb.clone(),
            kind: m.kind,
            video: false,
            staged: true,
            status: ItemStatus::Idle,
            auto_dismiss: true,
            saved: None,
        }
    }

    /// What a layout slot holding `action` does for this card.
    pub fn offered(&self, action: DeckAction) -> DeckAction {
        match action {
            DeckAction::Save if self.saved.is_some() => DeckAction::ShowInFolder,
            other => other,
        }
    }

    fn unsaved(&self) -> bool {
        (self.staged || self.video) && self.saved.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckAction {
    Pin,
    Dismiss,
    Copy,
    Save,
    Edit,
    Share,
    ShowInFolder,
    Preview,
    Delete,
}

impl DeckAction {
    /// The actions a layout places; Show in folder only ever stands in for Save.
    pub const ALL: [DeckAction; 8] = [
        DeckAction::Pin,
        DeckAction::Dismiss,
        DeckAction::Copy,
        DeckAction::Save,
        DeckAction::Edit,
        DeckAction::Share,
        DeckAction::Preview,
        DeckAction::Delete,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Dismissed,
    Evicted,
    Expired,
    Cleared,
    Copied,
    Saved,
    Edited,
    Pinned,
    /// The app moves the files to the Trash, and keeps the staging until Undo can no longer bring the card back.
    Deleted,
}

impl Removal {
    /// The capture was let go of, or handed on as a copy, so its staged files can be discarded.
    pub fn discards(self) -> bool {
        matches!(
            self,
            Removal::Dismissed
                | Removal::Evicted
                | Removal::Expired
                | Removal::Cleared
                | Removal::Copied
                | Removal::Edited
                | Removal::Pinned
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeckEvent {
    /// The app performs the action; Save answers with `saved` or `save_failed`.
    Requested {
        item: DeckItem,
        action: DeckAction,
    },
    Removed {
        item: DeckItem,
        why: Removal,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeckPolicy {
    /// `None` never dismisses.
    pub delay: Option<Duration>,
    pub keep_until_saved: bool,
}

impl Default for DeckPolicy {
    fn default() -> Self {
        Self::from_settings(&Overlay::default(), &CaptureCfg::default())
    }
}

impl DeckPolicy {
    pub fn from_settings(overlay: &Overlay, capture: &CaptureCfg) -> Self {
        let delay = (overlay.dismiss_delay < NEVER_DISMISS_SECS)
            .then(|| Duration::from_secs(u64::from(overlay.dismiss_delay.max(1))));
        DeckPolicy {
            delay,
            keep_until_saved: capture.keep_in_deck_until_saved,
        }
    }

    fn poll(self) -> Option<Duration> {
        self.delay.map(|d| d.min(POLL))
    }
}

struct Entry {
    item: DeckItem,
    idle: Duration,
}

/// The cards after a capture, oldest first.
#[derive(Default)]
pub struct Deck {
    entries: Vec<Entry>,
    policy: DeckPolicy,
    hovered: bool,
    hidden: bool,
    focused: Option<Uuid>,
    held: Option<Uuid>,
    shared: HashSet<Uuid>,
    ticking: bool,
}

impl EventEmitter<DeckEvent> for Deck {}

impl Deck {
    pub fn new(policy: DeckPolicy) -> Self {
        Deck {
            policy,
            ..Deck::default()
        }
    }

    pub fn items(&self) -> impl Iterator<Item = &DeckItem> {
        self.entries.iter().map(|e| &e.item)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: Uuid) -> Option<&DeckItem> {
        self.position(id).map(|i| &self.entries[i].item)
    }

    pub fn policy(&self) -> DeckPolicy {
        self.policy
    }

    pub fn can_save_all(&self) -> bool {
        self.items()
            .any(|i| i.unsaved() && i.status != ItemStatus::Saving)
    }

    fn position(&self, id: Uuid) -> Option<usize> {
        self.entries.iter().position(|e| e.item.id == id)
    }

    /// Showing a capture that is already here moves it to the newest place.
    pub fn show(&mut self, item: DeckItem, cx: &mut Context<Self>) {
        if let Some(i) = self.position(item.id) {
            self.entries.remove(i);
        }
        self.hidden = false;
        self.entries.push(Entry {
            item,
            idle: Duration::ZERO,
        });
        while self.entries.len() > MAX_ITEMS {
            let oldest = self
                .entries
                .iter()
                .position(|e| e.item.status != ItemStatus::Saving)
                .unwrap_or(0);
            let gone = self.entries.remove(oldest).item;
            cx.emit(DeckEvent::Removed {
                item: gone,
                why: Removal::Evicted,
            });
        }
        self.start_ticking(cx);
        cx.notify();
    }

    pub fn remove(&mut self, id: Uuid, why: Removal, cx: &mut Context<Self>) {
        if let Some(i) = self.position(id) {
            let item = self.entries.remove(i).item;
            if self.focused == Some(id) {
                self.focused = None;
            }
            if self.held == Some(id) {
                self.held = None;
            }
            self.shared.remove(&id);
            cx.emit(DeckEvent::Removed { item, why });
            cx.notify();
        }
    }

    pub fn act(&mut self, id: Uuid, action: DeckAction, cx: &mut Context<Self>) {
        let Some(item) = self.get(id).cloned() else {
            return;
        };
        if item.status == ItemStatus::Saving {
            return;
        }
        let action = item.offered(action);
        let gone = match action {
            DeckAction::Dismiss => Some(Removal::Dismissed),
            DeckAction::Copy if item.video => None,
            DeckAction::Copy => Some(Removal::Copied),
            DeckAction::Edit => Some(Removal::Edited),
            DeckAction::Pin => Some(Removal::Pinned),
            DeckAction::Delete => Some(Removal::Deleted),
            DeckAction::Save
            | DeckAction::Share
            | DeckAction::ShowInFolder
            | DeckAction::Preview => None,
        };
        if action == DeckAction::Save {
            self.set_status(id, ItemStatus::Saving, cx);
        }
        if action != DeckAction::Dismiss {
            cx.emit(DeckEvent::Requested {
                item: item.clone(),
                action,
            });
        }
        if let Some(why) = gone {
            self.remove(id, why, cx);
        }
    }

    /// A recording's Copy goes once the file is on the clipboard.
    pub fn copied(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.remove(id, Removal::Copied, cx);
    }

    pub fn saved(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.remove(id, Removal::Saved, cx);
    }

    /// The card stays, and no longer dismisses itself, so the capture is not lost.
    pub fn save_failed(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.set_status(id, ItemStatus::SaveFailed, cx);
    }

    pub fn save_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<Uuid> = self
            .items()
            .filter(|i| i.unsaved() && i.status != ItemStatus::Saving)
            .map(|i| i.id)
            .collect();
        for id in ids {
            self.act(id, DeckAction::Save, cx);
        }
    }

    /// Cards still saving stay until the save answers.
    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<Uuid> = self
            .items()
            .filter(|i| i.status != ItemStatus::Saving)
            .map(|i| i.id)
            .collect();
        for id in ids {
            self.remove(id, Removal::Cleared, cx);
        }
    }

    fn set_status(&mut self, id: Uuid, status: ItemStatus, cx: &mut Context<Self>) {
        if let Some(i) = self.position(id) {
            self.entries[i].item.status = status;
            self.entries[i].idle = Duration::ZERO;
            self.start_ticking(cx);
            cx.notify();
        }
    }

    pub fn set_policy(&mut self, policy: DeckPolicy, cx: &mut Context<Self>) {
        self.policy = policy;
        for e in &mut self.entries {
            e.idle = Duration::ZERO;
        }
        self.start_ticking(cx);
    }

    /// The pointer is anywhere over the deck; every timer waits.
    pub fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.hovered != hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }

    pub fn set_focused(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        if self.focused != id {
            self.focused = id;
            cx.notify();
        }
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    /// A popover opened from this card: the deck stays spread out and the card waits for it.
    pub fn hold(&mut self, id: Option<Uuid>, cx: &mut Context<Self>) {
        if self.held != id {
            self.held = id;
            self.start_ticking(cx);
            cx.notify();
        }
    }

    pub fn held(&self) -> Option<Uuid> {
        self.held
    }

    /// The capture has a live share link.
    pub fn set_shared(&mut self, id: Uuid, shared: bool, cx: &mut Context<Self>) {
        let changed = if shared {
            self.position(id).is_some() && self.shared.insert(id)
        } else {
            self.shared.remove(&id)
        };
        if changed {
            cx.notify();
        }
    }

    pub fn is_shared(&self, id: Uuid) -> bool {
        self.shared.contains(&id)
    }

    /// The cards fold into a stack and spread out while the pointer or the keyboard is on them.
    pub fn fanned(&self) -> bool {
        self.hovered || self.focused.is_some() || self.held.is_some()
    }

    /// The cards stay but are not shown, and their timers wait; a new capture shows the deck again.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Hiding closes the deck's surface, which never reports that the pointer or focus left it.
    pub fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.hidden = !self.hidden;
        if self.hidden {
            self.hovered = false;
            self.focused = None;
        }
        cx.notify();
    }

    fn dismisses(&self, item: &DeckItem) -> bool {
        item.auto_dismiss
            && self.policy.delay.is_some()
            && item.status == ItemStatus::Idle
            && !(self.policy.keep_until_saved && item.staged && !item.video && item.saved.is_none())
    }

    pub fn dismissing(&self, id: Uuid) -> bool {
        self.get(id).is_some_and(|i| self.dismisses(i))
    }

    fn any_dismissing(&self) -> bool {
        self.items().any(|i| self.dismisses(i))
    }

    /// Advances every running timer; one that waits paused restarts from zero, as the hover reads live state.
    pub fn tick(&mut self, elapsed: Duration, cx: &mut Context<Self>) {
        let Some(delay) = self.policy.delay else {
            return;
        };
        let mut expired = Vec::new();
        for i in 0..self.entries.len() {
            if !self.dismisses(&self.entries[i].item) {
                continue;
            }
            let entry = &mut self.entries[i];
            let id = Some(entry.item.id);
            if self.hovered || self.hidden || self.focused == id || self.held == id {
                entry.idle = Duration::ZERO;
                continue;
            }
            entry.idle += elapsed;
            if entry.idle >= delay {
                expired.push(entry.item.id);
            }
        }
        for id in expired {
            self.remove(id, Removal::Expired, cx);
        }
    }

    fn start_ticking(&mut self, cx: &mut Context<Self>) {
        let Some(poll) = self.policy.poll() else {
            return;
        };
        if self.ticking || !self.any_dismissing() {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |deck, cx| {
            loop {
                cx.background_executor().timer(poll).await;
                let more = deck.update(cx, |deck, cx| {
                    deck.tick(poll, cx);
                    let more = deck.any_dismissing() && deck.policy.poll() == Some(poll);
                    if !more {
                        deck.ticking = false;
                        deck.start_ticking(cx);
                    }
                    more
                });
                if !matches!(more, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }
}
