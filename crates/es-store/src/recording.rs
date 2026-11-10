use crate::atomic::{json_bytes, replace_atomic};
use crate::error::{Error, Result};
use crate::json::JsonFile;
use es_model::recording::{RecordingCapture, RecordingInput, RecordingProject};
use es_model::{Envelope, FormatKind, Header};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

pub const CAPTURE: &str = "capture.json";
pub const INPUT: &str = "input.json";
pub const PROJECT: &str = "project.json";

/// A JSON file in a recording folder. `capture.json` is written last: it marks the recording finished.
pub trait RecordingFile: Serialize + DeserializeOwned + Default {
    const NAME: &'static str;
    const KIND: FormatKind;
}

impl RecordingFile for RecordingCapture {
    const NAME: &'static str = CAPTURE;
    const KIND: FormatKind = FormatKind::Capture;
}

impl RecordingFile for RecordingInput {
    const NAME: &'static str = INPUT;
    const KIND: FormatKind = FormatKind::Input;
}

impl RecordingFile for RecordingProject {
    const NAME: &'static str = PROJECT;
    const KIND: FormatKind = FormatKind::Project;
}

/// `None` when the folder has no such file; a newer file opens read-only.
pub fn read<T: RecordingFile>(dir: &Path) -> Result<Option<JsonFile<T>>> {
    JsonFile::open_existing(&dir.join(T::NAME), T::KIND)
}

/// Never makes the folder: a folder removed while its recording finishes stays removed.
pub fn write<T: RecordingFile>(dir: &Path, value: &T) -> Result<()> {
    let path = dir.join(T::NAME);
    if let Some(version) = newer_on_disk(&path, T::KIND) {
        return Err(Error::NewerVersion { version });
    }
    replace_atomic(&path, &json_bytes(&Envelope::new(T::KIND, value))?)
}

fn newer_on_disk(path: &Path, kind: FormatKind) -> Option<u32> {
    let header: Header = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (header.format == kind.name() && header.version > kind.version()).then_some(header.version)
}
