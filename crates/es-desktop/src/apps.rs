use std::path::PathBuf;

/// The `Name=` of the desktop entry whose file name is the window's app id.
pub fn app_name(app_id: &str) -> Option<String> {
    app_name_in(&data_dirs(), app_id)
}

pub fn app_name_in(data_dirs: &[PathBuf], app_id: &str) -> Option<String> {
    if app_id.is_empty() || app_id.contains(['/', '\\']) || app_id.starts_with('.') {
        return None;
    }
    let files = [
        format!("{app_id}.desktop"),
        format!("{}.desktop", app_id.to_lowercase()),
    ];
    data_dirs
        .iter()
        .flat_map(|dir| files.iter().map(move |f| dir.join("applications").join(f)))
        .find_map(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| entry_name(&text))
}

fn entry_name(text: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if let Some(name) = line.strip_prefix("Name=").filter(|_| in_entry) {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_owned());
            }
        }
    }
    None
}

fn data_dirs() -> Vec<PathBuf> {
    let home = directories::BaseDirs::new().map(|d| d.data_dir().to_path_buf());
    let system = std::env::var_os("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    home.into_iter()
        .chain(std::env::split_paths(&system))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(file: &str, text: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        let apps = dir.path().join("applications");
        std::fs::create_dir(&apps).expect("applications dir");
        std::fs::write(apps.join(file), text).expect("desktop file");
        dir
    }

    #[test]
    fn the_name_comes_from_the_desktop_entry_section_only() {
        let dir = dir_with(
            "org.mozilla.firefox.desktop",
            "[Desktop Action new-window]\nName=New Window\n[Desktop Entry]\nName[de]=Feuerfuchs\nName=Firefox\n",
        );
        let dirs = [dir.path().to_path_buf()];
        assert_eq!(
            app_name_in(&dirs, "org.mozilla.firefox").as_deref(),
            Some("Firefox")
        );
        assert_eq!(app_name_in(&dirs, "org.example.missing"), None);
    }

    #[test]
    fn an_app_id_never_reaches_outside_the_applications_folder() {
        let dir = dir_with("x.desktop", "[Desktop Entry]\nName=X\n");
        let deeper = dir.path().join("deeper");
        std::fs::create_dir_all(deeper.join("applications")).expect("nested dir");
        let dirs = [deeper];
        assert_eq!(app_name_in(&dirs, "../../applications/x"), None);
    }
}
