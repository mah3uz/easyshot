use crate::errors::Result;
use es_model::HistoryEntry;
use es_store::Paths;
use gpui::{Context, EventEmitter};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEvent {
    Added(Uuid),
    Removed(Vec<Uuid>),
    Renamed(Uuid),
    ShareChanged(Uuid),
    Annotated(PathBuf),
    LimitChanged,
    VideosChanged,
}

pub struct History {
    inner: es_store::History,
}

impl EventEmitter<HistoryEvent> for History {}

impl History {
    pub fn open(paths: &Paths, limit: usize) -> Result<Self> {
        Ok(Self {
            inner: es_store::History::open(paths, limit)?,
        })
    }

    /// Newest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        self.inner.entries()
    }

    pub fn get(&self, id: Uuid) -> Option<&HistoryEntry> {
        self.inner.get(id)
    }

    pub fn by_file(&self, file: &Path) -> Option<&HistoryEntry> {
        self.inner.by_file(file)
    }

    /// Entries retention removed to make room are announced in a `Removed` event.
    pub fn add(&mut self, entry: HistoryEntry, cx: &mut Context<Self>) -> Result<()> {
        let id = entry.id;
        let gone = self.inner.add(entry)?;
        cx.emit(HistoryEvent::Added(id));
        self.announce_removed(&gone, cx);
        cx.notify();
        Ok(())
    }

    pub fn remove(&mut self, id: Uuid, cx: &mut Context<Self>) -> Result<bool> {
        let removed = self.inner.remove(id)?;
        if removed {
            cx.emit(HistoryEvent::Removed(vec![id]));
            cx.notify();
        }
        Ok(removed)
    }

    pub fn rename(&mut self, id: Uuid, name: &str, cx: &mut Context<Self>) -> Result<bool> {
        let renamed = self.inner.rename(id, name)?;
        if renamed {
            cx.emit(HistoryEvent::Renamed(id));
            cx.notify();
        }
        Ok(renamed)
    }

    pub fn set_share_url(
        &mut self,
        id: Uuid,
        url: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<bool> {
        let changed = self.inner.set_share_url(id, url)?;
        if changed {
            cx.emit(HistoryEvent::ShareChanged(id));
            cx.notify();
        }
        Ok(changed)
    }

    pub fn set_limit(&mut self, limit: usize, cx: &mut Context<Self>) -> Result<()> {
        let gone = self.inner.set_limit(limit)?;
        self.announce_removed(&gone, cx);
        cx.emit(HistoryEvent::LimitChanged);
        cx.notify();
        Ok(())
    }

    pub fn mark_annotated(&mut self, file: &Path, cx: &mut Context<Self>) -> Result<()> {
        self.inner.mark_annotated(file)?;
        cx.emit(HistoryEvent::Annotated(file.to_path_buf()));
        cx.notify();
        Ok(())
    }

    /// A video was saved, exported or deleted; the gallery and the tray read the video folders again.
    pub fn videos_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(HistoryEvent::VideosChanged);
        cx.notify();
    }

    fn announce_removed(&self, gone: &[HistoryEntry], cx: &mut Context<Self>) {
        if !gone.is_empty() {
            cx.emit(HistoryEvent::Removed(gone.iter().map(|e| e.id).collect()));
        }
    }
}
