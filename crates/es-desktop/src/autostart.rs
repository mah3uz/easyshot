use crate::{Error, Result};
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "easyshot.desktop";

pub fn autostart_file(config_home: &Path) -> PathBuf {
    config_home.join("autostart").join(FILE_NAME)
}

fn config_home() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|d| d.config_dir().to_path_buf())
        .ok_or_else(|| Error::NotFound("config directory".into()))
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub fn set_autostart_in(config_home: &Path, enabled: bool, exec: &str) -> Result<()> {
    let path = autostart_file(config_home);
    if !enabled {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(&path)(e)),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io(dir))?;
    }
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Easyshot\nExec={exec}\nIcon=dev.easyshot.Easyshot\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    );
    let tmp = path.with_extension("desktop.tmp");
    std::fs::write(&tmp, entry).map_err(io(&tmp))?;
    std::fs::rename(&tmp, &path).map_err(io(&path))
}

pub fn set_autostart(enabled: bool, exec: &str) -> Result<()> {
    set_autostart_in(&config_home()?, enabled, exec)
}

pub fn autostart_enabled() -> bool {
    config_home().is_ok_and(|home| autostart_file(&home).exists())
}
