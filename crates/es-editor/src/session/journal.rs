use es_model::{AnnotationDocument, FormatKind};
use es_store::{JsonFile, Paths};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use uuid::Uuid;

pub const DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub display: PathBuf,
    pub base_image: PathBuf,
    pub base_revision: u64,
    pub document: AnnotationDocument,
    /// The edited base pixels when they differ from `base_image`, as after a crop.
    pub staged_base: Option<PathBuf>,
}

/// A journal found on disk; its file name is the session it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    pub path: PathBuf,
    pub entry: Entry,
}

pub fn journal_path(paths: &Paths, session: &str) -> PathBuf {
    paths.recovery_dir().join(format!("{session}.json"))
}

pub fn session_of(path: &Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

/// Every write gets a fresh name, so the file a journal on disk references is never overwritten.
fn new_staged_path(paths: &Paths, session: &str) -> PathBuf {
    paths
        .recovery_dir()
        .join(format!("{session}.base-{}.png", Uuid::new_v4()))
}

/// The staged base files on disk for `session`, including any a crash left unreferenced.
pub fn staged_files(paths: &Paths, session: &str) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(paths.recovery_dir()) else {
        return Vec::new();
    };
    let prefix = format!("{session}.base-");
    dir.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_prefix(&prefix))
                .and_then(|rest| rest.strip_suffix(".png"))
                .is_some_and(|id| Uuid::parse_str(id).is_ok())
        })
        .collect()
}

pub fn write(paths: &Paths, session: &str, entry: Entry) -> es_store::Result<()> {
    es_store::atomic::ensure_private_dir(&paths.recovery_dir())?;
    let mut file = JsonFile::fresh(&journal_path(paths, session), FormatKind::Recovery);
    file.value = entry;
    file.save()
}

pub fn remove(paths: &Paths, session: &str) {
    remove_file(&journal_path(paths, session));
    for file in staged_files(paths, session) {
        remove_file(&file);
    }
}

fn remove_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "recovery file not removed");
    }
}

fn drop_session(paths: &Paths, journal: &Path) {
    if let Some(session) = session_of(journal) {
        for staged in staged_files(paths, &session) {
            remove_file(&staged);
        }
    }
    remove_file(journal);
}

pub fn load(path: &Path) -> es_store::Result<Option<Entry>> {
    Ok(JsonFile::<Entry>::open_existing(path, FormatKind::Recovery)?.map(|f| f.value))
}

/// Every readable journal in name order. One that cannot be parsed or migrated can never reopen,
/// so it and its staged bases are removed; one that failed to read from disk stays.
pub fn recover(paths: &Paths) -> Vec<Recovered> {
    let Ok(dir) = std::fs::read_dir(paths.recovery_dir()) else {
        return Vec::new();
    };
    let mut found: Vec<Recovered> = dir
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|path| match load(&path) {
            Ok(Some(entry)) => Some(Recovered { path, entry }),
            Ok(None) => None,
            Err(e) => {
                if matches!(e, es_store::Error::Io { .. }) {
                    tracing::warn!(error = %e, path = %path.display(), "recovery journal unreadable");
                } else {
                    tracing::warn!(error = %e, path = %path.display(), "recovery journal dropped");
                    drop_session(paths, &path);
                }
                None
            }
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

#[derive(Default)]
struct Written {
    generation: u64,
    /// The base revision the staged file holds, and the file.
    staged: Option<(u64, PathBuf)>,
}

/// One session's journal writes, in order: a write lands only when no newer one was asked for.
#[derive(Clone, Default)]
pub struct Journal(Arc<Mutex<Written>>);

/// A journal write that waits on encoding the base; run it off the main thread.
#[must_use]
pub struct Staging {
    journal: Journal,
    generation: u64,
    paths: Paths,
    session: String,
    entry: Entry,
    base: Arc<RgbaImage>,
}

impl Journal {
    fn lock(&self) -> MutexGuard<'_, Written> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes over a staged file a recovered journal references for the base at `revision`.
    pub fn adopt(&self, revision: u64, file: PathBuf) {
        self.lock().staged = Some((revision, file));
    }

    /// `edited` is the base when it differs from the file on disk. Writes now, unless those
    /// pixels are not staged yet; then the returned job encodes them and writes the journal.
    pub fn write(
        &self,
        paths: &Paths,
        session: &str,
        mut entry: Entry,
        edited: Option<Arc<RgbaImage>>,
    ) -> Option<Staging> {
        let mut w = self.lock();
        w.generation += 1;
        let staged = match (edited, &w.staged) {
            (None, _) => None,
            (Some(_), Some((revision, file))) if *revision == entry.base_revision => {
                Some((*revision, file.clone()))
            }
            (Some(base), _) => {
                return Some(Staging {
                    journal: self.clone(),
                    generation: w.generation,
                    paths: paths.clone(),
                    session: session.to_owned(),
                    entry,
                    base,
                });
            }
        };
        entry.staged_base = staged.as_ref().map(|(_, file)| file.clone());
        commit(&mut w, paths, session, &entry, staged);
        None
    }

    /// Deletes the journal and its staged base; a write still encoding is dropped.
    pub fn remove(&self, paths: &Paths, session: &str) {
        let mut w = self.lock();
        w.generation += 1;
        w.staged = None;
        remove(paths, session);
    }
}

impl Staging {
    pub fn run(mut self) {
        let mut png = std::io::Cursor::new(Vec::new());
        if let Err(error) = self.base.write_to(&mut png, image::ImageFormat::Png) {
            tracing::warn!(%error, "recovery base not encoded");
            return;
        }
        let mut w = self.journal.lock();
        if w.generation != self.generation {
            return;
        }
        let file = new_staged_path(&self.paths, &self.session);
        if let Err(error) = es_store::atomic::write_atomic(&file, png.get_ref()) {
            tracing::warn!(%error, "recovery base not written");
            return;
        }
        self.entry.staged_base = Some(file.clone());
        let revision = self.entry.base_revision;
        if !commit(
            &mut w,
            &self.paths,
            &self.session,
            &self.entry,
            Some((revision, file.clone())),
        ) {
            remove_file(&file);
        }
    }
}

/// The staged file must be fully on disk before the journal that references it, and an older
/// one goes only after the journal stops referencing it.
fn commit(
    w: &mut Written,
    paths: &Paths,
    session: &str,
    entry: &Entry,
    staged: Option<(u64, PathBuf)>,
) -> bool {
    if let Err(error) = write(paths, session, entry.clone()) {
        tracing::warn!(%error, "recovery journal not written");
        return false;
    }
    let keep = staged.as_ref().map(|(_, file)| file.clone());
    for file in staged_files(paths, session) {
        if Some(&file) != keep.as_ref() {
            remove_file(&file);
        }
    }
    w.staged = staged;
    true
}
