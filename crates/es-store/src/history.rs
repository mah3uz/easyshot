use crate::error::{Error, Result};
use crate::json::JsonFile;
use crate::library::{self, is_managed};
use crate::paths::Paths;
use crate::retention;
use es_model::{FormatKind, HistoryEntry, HistoryIndex};
use std::path::Path;
use uuid::Uuid;

pub struct History {
    paths: Paths,
    file: JsonFile<HistoryIndex>,
    limit: usize,
}

impl History {
    /// The index is a cache: a missing or unparsable one is rebuilt from the library.
    pub fn open(paths: &Paths, limit: usize) -> Result<Self> {
        let index = paths.history_index();
        let file = match JsonFile::open_existing(&index, FormatKind::History) {
            Ok(Some(file)) => file,
            Ok(None) => JsonFile::fresh(&index, FormatKind::History),
            Err(e) => {
                tracing::warn!(error = %e, "history index unreadable; rebuilding it");
                JsonFile::fresh(&index, FormatKind::History)
            }
        };
        let mut history = Self {
            paths: paths.clone(),
            file,
            limit,
        };
        history.reconcile();
        history.evict();
        history.persist()?;
        Ok(history)
    }

    fn reconcile(&mut self) {
        let entries = &mut self.file.value.entries;
        entries.retain(|e| e.file.exists() || e.share_url.is_some());
        for found in library::scan(&self.paths.library_dir()) {
            if !entries.iter().any(|e| e.file == found.file) {
                entries.push(found);
            }
        }
        entries.sort_by_key(|e| std::cmp::Reverse(e.created_at));
    }

    fn evict(&mut self) -> Vec<HistoryEntry> {
        let doomed = retention::excess(&self.file.value.entries, self.limit);
        let entries = &mut self.file.value.entries;
        let (gone, kept): (Vec<_>, Vec<_>) =
            entries.drain(..).partition(|e| doomed.contains(&e.id));
        *entries = kept;
        for entry in &gone {
            if is_managed(&self.paths, &entry.file) {
                library::remove_capture_files(&entry.file);
            }
        }
        gone
    }

    fn persist(&self) -> Result<()> {
        match self.file.save() {
            Err(Error::NewerVersion { .. }) => {
                tracing::debug!("history index is newer; not written");
                Ok(())
            }
            other => other,
        }
    }

    /// Newest first.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.file.value.entries
    }

    pub fn get(&self, id: Uuid) -> Option<&HistoryEntry> {
        self.entries().iter().find(|e| e.id == id)
    }

    pub fn by_file(&self, file: &Path) -> Option<&HistoryEntry> {
        self.entries().iter().find(|e| e.file == file)
    }

    /// Returns the entries retention removed to make room.
    pub fn add(&mut self, entry: HistoryEntry) -> Result<Vec<HistoryEntry>> {
        self.file.value.entries.retain(|e| e.id != entry.id);
        self.file.value.entries.push(entry);
        self.file
            .value
            .entries
            .sort_by_key(|e| std::cmp::Reverse(e.created_at));
        let gone = self.evict();
        self.persist()?;
        Ok(gone)
    }

    /// Deletes the capture's files when the library owns them.
    pub fn remove(&mut self, id: Uuid) -> Result<bool> {
        let entries = &mut self.file.value.entries;
        let Some(index) = entries.iter().position(|e| e.id == id) else {
            return Ok(false);
        };
        let entry = entries.remove(index);
        if is_managed(&self.paths, &entry.file) {
            library::remove_capture_files(&entry.file);
        }
        self.persist()?;
        Ok(true)
    }

    /// Changes the display name; the files keep their names.
    pub fn rename(&mut self, id: Uuid, name: &str) -> Result<bool> {
        let Some(entry) = self.file.value.entries.iter_mut().find(|e| e.id == id) else {
            return Ok(false);
        };
        entry.name = name.to_owned();
        self.persist()?;
        Ok(true)
    }

    pub fn set_share_url(&mut self, id: Uuid, url: Option<String>) -> Result<bool> {
        let Some(entry) = self.file.value.entries.iter_mut().find(|e| e.id == id) else {
            return Ok(false);
        };
        if entry.share_url == url {
            return Ok(false);
        }
        entry.share_url = url;
        self.persist()?;
        Ok(true)
    }

    pub fn set_limit(&mut self, limit: usize) -> Result<Vec<HistoryEntry>> {
        self.limit = limit;
        let gone = self.evict();
        self.persist()?;
        Ok(gone)
    }

    /// Records a committed annotated capture, adding the entry when it is new to the library.
    pub fn mark_annotated(&mut self, file: &Path) -> Result<()> {
        let Some(fresh) = library::entry_for_file(file) else {
            return Ok(());
        };
        match self.file.value.entries.iter_mut().find(|e| e.file == file) {
            Some(entry) => {
                entry.has_annotations = true;
                entry.pixel_size = fresh.pixel_size;
                self.persist()
            }
            None => self.add(fresh).map(|_| ()),
        }
    }
}
