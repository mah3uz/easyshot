use crate::atomic::write_atomic;
use crate::error::Result;
use crate::paths::Paths;
use std::path::PathBuf;

/// A document refers to its wallpaper by path, so a built-in one is written out once and kept.
pub fn install_built_in(paths: &Paths, file_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let path = paths.wallpapers_dir().join(file_name);
    let current = std::fs::metadata(&path).map(|m| m.len()).ok();
    if current != Some(bytes.len() as u64) {
        write_atomic(&path, bytes)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_built_in_wallpaper_is_written_once_and_repaired_when_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        let path = install_built_in(&paths, "wall-1.jpg", b"jpeg bytes").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"jpeg bytes");
        std::fs::write(&path, b"jp").unwrap();
        install_built_in(&paths, "wall-1.jpg", b"jpeg bytes").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"jpeg bytes");
    }
}
