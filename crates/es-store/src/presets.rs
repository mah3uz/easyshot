use crate::atomic::write_atomic;
use crate::error::{Error, Result, io};
use crate::json::JsonFile;
use crate::paths::Paths;
use es_model::{FormatKind, PresetLibrary, StylePreset};

pub type StyleFile = JsonFile<StylePreset>;
pub type BackgroundPresetFile = JsonFile<PresetLibrary>;

pub fn open_style(paths: &Paths) -> Result<StyleFile> {
    JsonFile::open(&paths.style_file(), FormatKind::Style)
}

/// A library that does not parse is copied to the `.recovery` file and replaced by an empty one,
/// so a damaged file never blocks the editor and is never lost.
pub fn open_backgrounds(paths: &Paths) -> Result<BackgroundPresetFile> {
    let path = paths.bg_presets_file();
    match JsonFile::open(&path, FormatKind::BgPresets) {
        Err(Error::Parse { .. } | Error::WrongFormat { .. }) => {
            let bytes = std::fs::read(&path).map_err(io(&path))?;
            write_atomic(&paths.bg_presets_recovery_file(), &bytes)?;
            tracing::warn!("background presets did not parse; kept a copy for recovery");
            Ok(JsonFile::fresh(&path, FormatKind::BgPresets))
        }
        other => other,
    }
}
