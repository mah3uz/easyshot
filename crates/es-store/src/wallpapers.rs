use crate::atomic::write_atomic;
use crate::error::Result;
use crate::paths::Paths;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MAX_BYTES: u64 = 64 << 20;

/// A document refers to its wallpaper by path, so a built-in one is written out once and kept.
pub fn install_built_in(paths: &Paths, file_name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let path = paths.wallpapers_dir().join(file_name);
    let current = std::fs::metadata(&path).map(|m| m.len()).ok();
    if current != Some(bytes.len() as u64) {
        write_atomic(&path, bytes)?;
    }
    Ok(path)
}

/// A document from disk may name any path; only stored or user-picked wallpapers are drawn.
pub fn trusted(paths: &Paths, path: &Path) -> bool {
    let in_store = match (path.canonicalize(), paths.wallpapers_dir().canonicalize()) {
        (Ok(file), Ok(store)) => file.parent() == Some(store.as_path()),
        _ => false,
    };
    in_store
        || crate::state::open(paths)
            .is_ok_and(|s| s.value.recent_wallpapers.iter().any(|p| p == path))
}

/// Non-blocking and no-follow, so a device, a FIFO, a link or a huge file reads as missing.
pub fn read(path: &Path) -> Option<Vec<u8>> {
    use rustix::fs::{FileType, Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).ok()?;
    let stat = rustix::fs::fstat(&fd).ok()?;
    let size = u64::try_from(stat.st_size).ok()?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || size > MAX_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_BYTES).then_some(bytes)
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

    fn store_with(file: &str) -> (tempfile::TempDir, Paths, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        let path = install_built_in(&paths, file, b"jpeg bytes").unwrap();
        (dir, paths, path)
    }

    #[test]
    fn only_the_store_and_picked_files_are_trusted() {
        let (dir, paths, inside) = store_with("wall-1.jpg");
        assert!(trusted(&paths, &inside));
        let outside = dir.path().join("private.png");
        std::fs::write(&outside, b"secret").unwrap();
        assert!(!trusted(&paths, &outside));
        assert!(!trusted(&paths, Path::new("/dev/zero")));
        let link = paths.wallpapers_dir().join("link.png");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(!trusted(&paths, &link), "a link out of the store");
        let escape = paths.wallpapers_dir().join("..").join("private.png");
        assert!(!trusted(&paths, &escape));

        let mut state = crate::state::open(&paths).unwrap();
        state.value.push_recent_wallpaper(outside.clone());
        state.save().unwrap();
        assert!(trusted(&paths, &outside), "the user picked it");
    }

    #[test]
    fn only_a_bounded_regular_file_is_read() {
        use rustix::fs::{CWD, FileType, Mode, mknodat};
        let (dir, _paths, inside) = store_with("wall-1.jpg");
        assert_eq!(read(&inside).unwrap(), b"jpeg bytes");
        assert!(read(Path::new("/dev/zero")).is_none());
        let fifo = dir.path().join("fifo.png");
        mknodat(CWD, &fifo, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
        assert!(read(&fifo).is_none());
        let link = dir.path().join("link.jpg");
        std::os::unix::fs::symlink(&inside, &link).unwrap();
        assert!(read(&link).is_none());
        let huge = dir.path().join("huge.png");
        std::fs::File::create(&huge)
            .unwrap()
            .set_len(MAX_BYTES + 1)
            .unwrap();
        assert!(read(&huge).is_none());
        assert!(read(&dir.path().join("missing.png")).is_none());
    }
}
