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

/// The temp file lives in the target's directory so the rename never crosses a filesystem.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    ensure_private_dir(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".easyshot-tmp-")
        .tempfile_in(dir)
        .map_err(io(dir))?;
    tmp.write_all(bytes).map_err(io(path))?;
    tmp.as_file().sync_all().map_err(io(path))?;
    tmp.persist(path).map_err(|e| io(path)(e.error))?;
    File::open(dir).and_then(|d| d.sync_all()).map_err(io(dir))
}

pub fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}
