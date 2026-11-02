use crate::error::{Error, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_millis(150);

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    /// Watches the parent directory because atomic writes replace the file's inode.
    /// `on_change` runs on a helper thread, once per burst of events.
    pub fn new(path: &Path, on_change: impl Fn() + Send + 'static) -> Result<Self> {
        let name = path
            .file_name()
            .ok_or_else(|| Error::Watch("path has no file name".into()))?
            .to_owned();
        let dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let (tx, rx) = mpsc::channel::<()>();
        let mut watcher = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
            if let Ok(ev) = ev
                && !ev.kind.is_access()
                && ev.paths.iter().any(|p| p.file_name() == Some(&name))
            {
                let _ = tx.send(());
            }
        })
        .map_err(|e| Error::Watch(e.to_string()))?;
        watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| Error::Watch(e.to_string()))?;
        std::thread::Builder::new()
            .name("config-watch".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.recv_timeout(DEBOUNCE).is_ok() {}
                    on_change();
                }
            })
            .map_err(|e| Error::Watch(e.to_string()))?;
        Ok(Self { _watcher: watcher })
    }
}
