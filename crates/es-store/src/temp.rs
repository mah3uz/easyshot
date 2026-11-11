use crate::atomic::{ensure_private_dir, write_atomic};
use crate::error::{Result, io};
use crate::paths::Paths;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempKind {
    Clipboard,
    Drag,
    Unstaged,
    Share,
    VideoCopy,
}

fn root(paths: &Paths, kind: TempKind) -> PathBuf {
    match kind {
        TempKind::Clipboard => paths.clipboard_dir(),
        TempKind::Drag => paths.drag_dir(),
        TempKind::Unstaged => paths.unstaged_dir(),
        TempKind::Share => paths.share_dir(),
        TempKind::VideoCopy => paths.video_copies_dir(),
    }
}

/// Writes `<root>/<uuid>/<name>`; the uuid directory keeps the user-visible file name intact.
pub fn create(paths: &Paths, kind: TempKind, file_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let name = Path::new(file_name)
        .file_name()
        .map_or_else(|| "capture".into(), |n| n.to_os_string());
    let dir = root(paths, kind).join(Uuid::new_v4().to_string());
    ensure_private_dir(&dir)?;
    let file = dir.join(name);
    write_atomic(&file, bytes)?;
    Ok(file)
}

/// An empty private `<root>/<uuid>` for files a job writes itself; the caller removes it.
pub fn make_dir(paths: &Paths, kind: TempKind) -> Result<PathBuf> {
    let dir = root(paths, kind).join(Uuid::new_v4().to_string());
    ensure_private_dir(&dir)?;
    Ok(dir)
}

/// Removes item directories not modified for a day; returns how many went.
pub fn purge(paths: &Paths, now: SystemTime) -> Result<usize> {
    let mut removed = 0;
    for kind in [
        TempKind::Clipboard,
        TempKind::Drag,
        TempKind::Unstaged,
        TempKind::Share,
        TempKind::VideoCopy,
    ] {
        let dir = root(paths, kind);
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io(&dir)(e)),
        };
        for item in read.filter_map(|e| e.ok()) {
            let Ok(modified) = item.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            let age = now.duration_since(modified).unwrap_or_default();
            if age >= MAX_AGE && std::fs::remove_dir_all(item.path()).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}
