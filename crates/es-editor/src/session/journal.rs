use es_model::{AnnotationDocument, FormatKind};
use es_store::{JsonFile, Paths};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub display: PathBuf,
    pub base_image: PathBuf,
    pub base_revision: u64,
    pub document: AnnotationDocument,
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

pub fn write(paths: &Paths, session: &str, entry: Entry) -> es_store::Result<()> {
    es_store::atomic::ensure_private_dir(&paths.recovery_dir())?;
    let mut file = JsonFile::fresh(&journal_path(paths, session), FormatKind::Recovery);
    file.value = entry;
    file.save()
}

pub fn remove(paths: &Paths, session: &str) {
    let path = journal_path(paths, session);
    if let Err(e) = std::fs::remove_file(&path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(error = %e, path = %path.display(), "recovery journal not removed");
    }
}

pub fn load(path: &Path) -> es_store::Result<Option<Entry>> {
    Ok(JsonFile::<Entry>::open_existing(path, FormatKind::Recovery)?.map(|f| f.value))
}

/// Every readable journal in name order; an unreadable one stays on disk and is skipped.
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
                tracing::warn!(error = %e, path = %path.display(), "recovery journal unreadable");
                None
            }
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}
