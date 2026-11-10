use crate::error::{Error, Result, io};
use std::fs::{DirBuilder, File};
use std::io::Write;
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

pub fn ensure_private_dir(dir: &Path) -> Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io(dir))
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    ensure_private_dir(parent(path))?;
    replace_atomic(path, bytes)
}

/// The temp file lives in the target's directory so the rename never crosses a filesystem.
/// Fails when that directory is gone instead of making it again.
pub(crate) fn replace_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = parent(path);
    let mut tmp = tempfile::Builder::new()
        .prefix(".easyshot-tmp-")
        .tempfile_in(dir)
        .map_err(io(dir))?;
    tmp.write_all(bytes).map_err(io(path))?;
    tmp.as_file().sync_all().map_err(io(path))?;
    tmp.persist(path).map_err(|e| io(path)(e.error))?;
    File::open(dir).and_then(|d| d.sync_all()).map_err(io(dir))
}

pub(crate) fn json_bytes<T: serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    write_atomic(path, &json_bytes(value)?)
}
