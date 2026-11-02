use crate::document::sidecar_path;
use crate::paths::Paths;
use es_model::{CaptureKind, HistoryEntry};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

/// `shot.png` -> `shot.base.png`.
pub fn base_path(image: &Path) -> PathBuf {
    let mut name: OsString = image.file_stem().map(OsString::from).unwrap_or_default();
    name.push(".base");
    if let Some(ext) = image.extension() {
        name.push(".");
        name.push(ext);
    }
    image.with_file_name(name)
}

fn is_base(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.ends_with(".base"))
}

pub fn is_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    matches!(ext.as_deref(), Some("png" | "jpg" | "jpeg")) && !is_base(path)
}

/// Files the library owns; a capture saved elsewhere is only referenced and never deleted.
pub fn is_managed(paths: &Paths, file: &Path) -> bool {
    file.starts_with(paths.library_dir())
}

pub fn remove_capture_files(image: &Path) {
    for path in [image.to_path_buf(), sidecar_path(image), base_path(image)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "could not delete a capture file"),
        }
    }
}

pub fn entry_for_file(file: &Path) -> Option<HistoryEntry> {
    let pixel_size = image::image_dimensions(file).ok()?;
    let modified = std::fs::metadata(file).and_then(|m| m.modified()).ok()?;
    let name = file.file_stem()?.to_string_lossy().into_owned();
    // A name-derived id keeps a rebuilt index stable between scans.
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, file.file_name()?.as_encoded_bytes());
    Some(HistoryEntry {
        id,
        created_at: OffsetDateTime::from(modified),
        kind: CaptureKind::Region,
        name,
        file: file.to_path_buf(),
        export_path: None,
        pixel_size,
        has_annotations: sidecar_path(file).exists(),
        share_url: None,
    })
}

pub fn scan(dir: &Path) -> Vec<HistoryEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_image(p))
        .filter_map(|p| entry_for_file(&p))
        .collect()
}
