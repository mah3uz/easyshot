use crate::error::Result;
use crate::json::JsonFile;
use crate::paths::Paths;
use es_model::{FormatKind, State};

pub type StateFile = JsonFile<State>;

pub fn open(paths: &Paths) -> Result<StateFile> {
    JsonFile::open(&paths.state_file(), FormatKind::State)
}
