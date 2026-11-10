use crate::atomic::write_atomic;
use crate::error::Result;
use crate::paths::Paths;
use std::path::PathBuf;

/// Replaces easyshot's compositor binds file; the compositor config itself is never written.
pub fn write(paths: &Paths, name: &str, text: &str) -> Result<PathBuf> {
    let file = paths.binds_file(name);
    write_atomic(&file, text.as_bytes())?;
    Ok(file)
}

/// The text of easyshot's binds file, or `None` when there is none yet.
pub fn read(paths: &Paths, name: &str) -> Option<String> {
    std::fs::read_to_string(paths.binds_file(name)).ok()
}
