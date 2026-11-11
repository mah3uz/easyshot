use crate::atomic::write_atomic;
use crate::error::{Result, io};
use crate::json::JsonFile;
use crate::paths::Paths;
use es_model::{BucketKeys, FormatKind, Shares};
use std::path::Path;

pub type SharesFile = JsonFile<Shares>;

pub fn open(paths: &Paths) -> Result<SharesFile> {
    JsonFile::open(&paths.shares_file(), FormatKind::Shares)
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(path)(e)),
        _ => Ok(()),
    }
}

pub fn read_install_token(paths: &Paths) -> Result<Option<String>> {
    let path = paths.share_install_token_file();
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text.trim().to_owned()).filter(|t| !t.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(&path)(e)),
    }
}

pub fn write_install_token(paths: &Paths, token: &str) -> Result<()> {
    write_atomic(&paths.share_install_token_file(), token.as_bytes())
}

pub fn clear_install_token(paths: &Paths) -> Result<()> {
    remove_if_present(&paths.share_install_token_file())
}

pub fn read_bucket_keys(paths: &Paths) -> Result<Option<BucketKeys>> {
    let path = paths.bucket_keys_file();
    Ok(JsonFile::<BucketKeys>::open_existing(&path, FormatKind::BucketKeys)?.map(|f| f.value))
}

pub fn write_bucket_keys(paths: &Paths, keys: BucketKeys) -> Result<()> {
    let mut file = JsonFile::<BucketKeys>::open(&paths.bucket_keys_file(), FormatKind::BucketKeys)?;
    file.value = keys;
    file.save()
}

pub fn clear_bucket_keys(paths: &Paths) -> Result<()> {
    remove_if_present(&paths.bucket_keys_file())
}
