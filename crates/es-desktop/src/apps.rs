use std::path::{Path, PathBuf};

const ICON_SIZE: u32 = 32;

/// How a window's app reads in a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppLabel {
    pub name: String,
    /// A PNG or SVG file.
    pub icon: Option<PathBuf>,
}

/// The `Name=` of the window's desktop entry, else a name made from the app id.
pub fn app_name(app_id: &str) -> String {
    app_name_in(&data_dirs(), app_id).unwrap_or_else(|| fallback_name(app_id))
}

/// The name and icon of the window's app; reads the disk, so callers keep the answer.
pub fn app_label(app_id: &str) -> AppLabel {
    app_label_in(&data_dirs(), &icon_themes(), app_id)
}

pub fn app_name_in(data_dirs: &[PathBuf], app_id: &str) -> Option<String> {
    entry_in(data_dirs, app_id).and_then(|text| value(&text, "[Desktop Entry]", "Name"))
}

/// `themes` in the order they are searched, `hicolor` last.
pub fn app_label_in(data_dirs: &[PathBuf], themes: &[String], app_id: &str) -> AppLabel {
    let entry = entry_in(data_dirs, app_id);
    let key = |key| {
        entry
            .as_deref()
            .and_then(|text| value(text, "[Desktop Entry]", key))
    };
    AppLabel {
        name: key("Name").unwrap_or_else(|| fallback_name(app_id)),
        icon: key("Icon").and_then(|icon| icon_in(data_dirs, themes, &icon)),
    }
}

/// The last part of a reverse-domain id, without a random suffix, capitalised:
/// `com.example.vscode-x8f2k1` reads "Vscode", `google-chrome` reads "Google Chrome".
pub fn fallback_name(app_id: &str) -> String {
    let last = app_id.rsplit('.').next().unwrap_or(app_id);
    let mut words = last.split(['-', '_']).filter(|w| !w.is_empty());
    let first = words.next();
    first
        .into_iter()
        .chain(words.filter(|w| !random(w)))
        .map(capitalised)
        .collect::<Vec<_>>()
        .join(" ")
}

fn random(word: &str) -> bool {
    word.len() >= 16
        || word.chars().any(|c| c.is_ascii_digit())
        || !word.chars().any(|c| "aeiouyAEIOUY".contains(c))
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn safe(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && !name.starts_with('.')
}

/// The entry named after the app id, else the one whose `StartupWMClass=` is the app id: many
/// apps' Wayland app id differs from their desktop file's name.
fn entry_in(data_dirs: &[PathBuf], app_id: &str) -> Option<String> {
    if !safe(app_id) {
        return None;
    }
    let files = [
        format!("{app_id}.desktop"),
        format!("{}.desktop", app_id.to_lowercase()),
    ];
    let apps = || data_dirs.iter().map(|dir| dir.join("applications"));
    apps()
        .flat_map(|dir| files.iter().map(move |f| dir.join(f)))
        .find_map(|path| std::fs::read_to_string(path).ok())
        .or_else(|| {
            apps()
                .filter_map(|dir| std::fs::read_dir(dir).ok())
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "desktop"))
                .filter_map(|p| std::fs::read_to_string(p).ok())
                .find(|text| {
                    value(text, "[Desktop Entry]", "StartupWMClass")
                        .is_some_and(|class| class.eq_ignore_ascii_case(app_id))
                })
        })
}

fn value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == section;
        } else if let Some((k, v)) = line.split_once('=').filter(|_| inside) {
            let v = v.trim().trim_matches('"');
            if k.trim() == key && !v.is_empty() {
                return Some(v.to_owned());
            }
        }
    }
    None
}

fn drawable(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x.eq_ignore_ascii_case("png") || x.eq_ignore_ascii_case("svg"))
}

/// An absolute `Icon=` as is; a name from the themes, then `pixmaps`.
fn icon_in(data_dirs: &[PathBuf], themes: &[String], icon: &str) -> Option<PathBuf> {
    let path = Path::new(icon);
    if path.is_absolute() {
        return drawable(path).then(|| path.to_path_buf());
    }
    if !safe(icon) {
        return None;
    }
    let name = [".png", ".svg"]
        .iter()
        .find_map(|x| icon.strip_suffix(x))
        .unwrap_or(icon);
    themes
        .iter()
        .filter(|theme| safe(theme))
        .find_map(|theme| {
            data_dirs
                .iter()
                .find_map(|dir| themed(&dir.join("icons").join(theme), name))
        })
        .or_else(|| {
            data_dirs
                .iter()
                .find_map(|dir| file_in(&dir.join("pixmaps"), name))
        })
}

fn file_in(dir: &Path, name: &str) -> Option<PathBuf> {
    ["png", "svg"]
        .iter()
        .map(|x| dir.join(format!("{name}.{x}")))
        .find(|p| drawable(p))
}

/// `scalable/apps` first, then the smallest size above 32, then the largest below it.
fn themed(theme: &Path, name: &str) -> Option<PathBuf> {
    let scalable = theme.join("scalable/apps").join(format!("{name}.svg"));
    if drawable(&scalable) {
        return Some(scalable);
    }
    let mut sized: Vec<(u32, PathBuf)> = std::fs::read_dir(theme)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let size = e.file_name().to_str()?.split_once('x')?.0.parse().ok()?;
            Some((size, file_in(&e.path().join("apps"), name)?))
        })
        .collect();
    sized.sort_by_key(|&(size, _)| (size <= ICON_SIZE, size.abs_diff(ICON_SIZE)));
    sized.into_iter().next().map(|(_, path)| path)
}

/// The user's GTK icon theme when it says one, then `hicolor`.
fn icon_themes() -> Vec<String> {
    let config = directories::BaseDirs::new().map(|d| d.config_dir().to_path_buf());
    let user = config.and_then(|config| {
        ["gtk-3.0", "gtk-4.0"].iter().find_map(|gtk| {
            let text = std::fs::read_to_string(config.join(gtk).join("settings.ini")).ok()?;
            value(&text, "[Settings]", "gtk-icon-theme-name")
        })
    });
    user.into_iter()
        .filter(|theme| theme != "hicolor")
        .chain(["hicolor".to_owned()])
        .collect()
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
#[path = "apps_tests.rs"]
mod tests;
