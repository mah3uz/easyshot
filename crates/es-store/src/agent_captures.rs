use crate::atomic::write_atomic;
use crate::error::{Result, io};
use crate::paths::Paths;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

pub const KEEP: usize = 200;

/// Writes a capture the user's library does not keep, then drops all but the newest `KEEP`.
pub fn write(paths: &Paths, bytes: &[u8]) -> Result<PathBuf> {
    let dir = paths.agent_dir();
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let name = format!(
        "agent-{stamp}-{}.png",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    let file = dir.join(name);
    write_atomic(&file, bytes)?;
    prune(&dir, KEEP)?;
    Ok(file)
}

/// Removes every file left by an earlier run.
pub fn clear(paths: &Paths) -> Result<()> {
    let dir = paths.agent_dir();
    let read = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io(&dir)(e)),
    };
    for item in read.filter_map(|e| e.ok()) {
        let _ = std::fs::remove_file(item.path());
    }
    Ok(())
}

fn prune(dir: &Path, keep: usize) -> Result<()> {
    let read = std::fs::read_dir(dir).map_err(io(dir))?;
    let mut files: Vec<(SystemTime, PathBuf)> = read
        .filter_map(|e| e.ok())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.cmp(a));
    for (_, path) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn files(paths: &Paths) -> Vec<PathBuf> {
        std::fs::read_dir(paths.agent_dir())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    #[test]
    fn only_the_newest_files_stay_so_bulk_checks_do_not_fill_the_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(tmp.path());
        paths.ensure_all().unwrap();
        let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let mut made = Vec::new();
        for n in 0..203u64 {
            let file = paths.agent_dir().join(format!("{n}.png"));
            std::fs::write(&file, b"x").unwrap();
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(base + Duration::from_secs(n))
                .unwrap();
            prune(&paths.agent_dir(), KEEP).unwrap();
            made.push(file);
        }
        let mut left = files(&paths);
        left.sort();
        let mut want: Vec<PathBuf> = made[3..].to_vec();
        want.sort();
        assert_eq!(left, want);
    }

    #[test]
    fn write_lands_in_the_agent_folder_and_clear_empties_it() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(tmp.path());
        paths.ensure_all().unwrap();
        let file = write(&paths, b"png").unwrap();
        assert_eq!(file.parent().unwrap(), paths.agent_dir());
        assert_eq!(std::fs::read(&file).unwrap(), b"png");
        clear(&paths).unwrap();
        assert!(files(&paths).is_empty());
    }
}
