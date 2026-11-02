use crate::paths::Paths;
use sha1::{Digest, Sha1};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The cache name changes with the file's modification time, so an edited image never serves a stale thumbnail.
pub fn thumb_path(paths: &Paths, image: &Path, modified: SystemTime, size: u32) -> PathBuf {
    let nanos = modified
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut hasher = Sha1::new();
    hasher.update(image.as_os_str().as_encoded_bytes());
    hasher.update(nanos.to_string().as_bytes());
    let mut hex = String::with_capacity(40);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    paths.thumbs_dir().join(format!("{hex}-{size}.png"))
}

pub fn thumb_path_for(paths: &Paths, image: &Path, size: u32) -> Option<PathBuf> {
    let modified = std::fs::metadata(image).and_then(|m| m.modified()).ok()?;
    Some(thumb_path(paths, image, modified, size))
}
