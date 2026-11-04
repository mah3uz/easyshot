use crate::atomic::write_atomic;
use crate::error::{Error, Result, io};
use crate::json::JsonFile;
use crate::paths::Paths;
use es_model::{FormatKind, GradientLibrary, PresetLibrary, StylePreset};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub type StyleFile = JsonFile<StylePreset>;
pub type BackgroundPresetFile = JsonFile<PresetLibrary>;
pub type GradientFile = JsonFile<GradientLibrary>;

pub fn open_style(paths: &Paths) -> Result<StyleFile> {
    JsonFile::open(&paths.style_file(), FormatKind::Style)
}

/// A library that does not parse is copied to the `.recovery` file and replaced by an empty one,
/// so a damaged file never blocks the editor and is never lost.
fn open_library<T: Serialize + DeserializeOwned + Default>(
    path: &Path,
    recovery: &Path,
    kind: FormatKind,
) -> Result<JsonFile<T>> {
    match JsonFile::open(path, kind) {
        Err(Error::Parse { .. } | Error::WrongFormat { .. }) => {
            let bytes = std::fs::read(path).map_err(io(path))?;
            write_atomic(recovery, &bytes)?;
            tracing::warn!(
                format = kind.name(),
                "a library did not parse; kept a copy for recovery"
            );
            Ok(JsonFile::fresh(path, kind))
        }
        other => other,
    }
}

pub fn open_backgrounds(paths: &Paths) -> Result<BackgroundPresetFile> {
    open_library(
        &paths.bg_presets_file(),
        &paths.bg_presets_recovery_file(),
        FormatKind::BgPresets,
    )
}

pub fn open_gradients(paths: &Paths) -> Result<GradientFile> {
    open_library(
        &paths.gradients_file(),
        &paths.gradients_recovery_file(),
        FormatKind::Gradients,
    )
}
