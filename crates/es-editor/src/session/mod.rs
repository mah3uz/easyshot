pub mod crop;
pub mod doc;
pub mod io;
pub mod look;
pub mod viewport;

use crate::Result;
use crate::session::{
    crop::CropModel, doc::DocModel, io::EditorIo, look::LookModel, viewport::ViewportModel,
};
use gpui::{App, Entity, Task};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum EditorSource {
    History(Uuid),
    File(PathBuf),
    /// `DeckManifest.id`.
    Staged(Uuid),
    Recovery(PathBuf),
}

pub struct EditorSession {
    pub doc: Entity<DocModel>,
    pub viewport: Entity<ViewportModel>,
    pub look: Entity<LookModel>,
    pub crop: Entity<CropModel>,
    pub io: Entity<EditorIo>,
}

impl EditorSession {
    pub fn open(_source: EditorSource, _cx: &mut App) -> Task<Result<Self>> {
        todo!()
    }

    /// Engine stack first, then the image stack.
    pub fn undo(&self, _cx: &mut App) {
        todo!()
    }

    /// Engine stack first, then the image stack.
    pub fn redo(&self, _cx: &mut App) {
        todo!()
    }
}
