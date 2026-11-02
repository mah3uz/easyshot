use crate::json::JsonFile;
use es_model::{AnnotationDocument, FormatKind};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub type SidecarFile = JsonFile<AnnotationDocument>;

pub fn sidecar_path(image: &Path) -> PathBuf {
    let mut name: OsString = image.as_os_str().to_owned();
    name.push(".easyshot");
    PathBuf::from(name)
}

/// None when there is no sidecar or it cannot be read; a newer one opens read-only.
pub fn open(image: &Path) -> Option<SidecarFile> {
    match JsonFile::open_existing(&sidecar_path(image), FormatKind::Document) {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!(error = %e, "annotation sidecar unreadable");
            None
        }
    }
}

pub fn create(image: &Path, document: AnnotationDocument) -> SidecarFile {
    let mut file = JsonFile::fresh(&sidecar_path(image), FormatKind::Document);
    file.value = document;
    file
}
