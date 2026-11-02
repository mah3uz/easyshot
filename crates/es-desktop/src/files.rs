use crate::bus::{connect, protocol};
use crate::dbus::Endpoint;
use crate::{Error, Result};
use std::path::Path;

#[zbus::proxy(
    interface = "org.freedesktop.FileManager1",
    default_service = "org.freedesktop.FileManager1",
    default_path = "/org/freedesktop/FileManager1"
)]
trait FileManager {
    fn show_items(&self, uris: &[&str], startup_id: &str) -> zbus::Result<()>;
}

pub fn move_to_trash(path: &Path) -> Result<()> {
    if matches!(path.symlink_metadata(), Err(e) if e.kind() == std::io::ErrorKind::NotFound) {
        return Err(Error::NotFound(path.display().to_string()));
    }
    trash::delete(path).map_err(|e| Error::Protocol(e.to_string()))
}

fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
            uri.push(char::from(b));
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

pub async fn show_items(endpoint: &Endpoint, path: &Path) -> Result<()> {
    let conn = connect(endpoint).await?;
    FileManagerProxy::new(&conn)
        .await
        .map_err(protocol)?
        .show_items(&[file_uri(path).as_str()], "")
        .await
        .map_err(protocol)
}

fn open_folder(path: &Path) -> Result<()> {
    let folder = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open")
        .arg(folder)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|source| Error::Io {
            path: "xdg-open".into(),
            source,
        })
}

pub async fn show_in_file_manager(endpoint: &Endpoint, path: &Path) -> Result<()> {
    match show_items(endpoint, path).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::debug!("FileManager1.ShowItems failed ({e}); opening the folder");
            open_folder(path)
        }
    }
}
