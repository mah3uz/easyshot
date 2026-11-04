use crate::strings::fl;
use std::path::Path;

pub fn no_daemon() -> String {
    fl!("agent-hint-no-daemon")
}

pub fn file_missing(path: &Path) -> String {
    fl!("agent-hint-file-missing", path = path.display().to_string())
}

pub fn not_an_image(path: &Path) -> String {
    fl!("agent-hint-not-an-image", path = path.display().to_string())
}
