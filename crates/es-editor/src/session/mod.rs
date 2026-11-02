pub mod crop;
pub mod doc;
pub mod io;
pub mod journal;
pub mod look;
pub mod viewport;

use crate::session::{
    crop::CropModel,
    doc::{DocModel, Opened},
    io::EditorIo,
    look::LookModel,
    viewport::ViewportModel,
};
use crate::{Error, Result};
use es_anno::BundledText;
use es_app::services::Services;
use es_model::StylePreset;
use gpui::{App, AppContext, Entity, Task};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

/// Reads the sidecar and decodes the base image; a plain capture opens with an empty document.
pub fn load(display: &Path) -> Result<Opened> {
    let reopened = es_store::reopen(display);
    let base = image::open(&reopened.source)
        .map_err(|e| match e {
            image::ImageError::IoError(source) => Error::Io {
                path: reopened.source.clone(),
                source,
            },
            other => Error::Protocol(format!("{}: {other}", reopened.source.display())),
        })?
        .to_rgba8();
    Ok(Opened {
        display: reopened.display,
        base,
        document: reopened.document.unwrap_or_default(),
        read_only: reopened.read_only,
    })
}

impl EditorSession {
    pub fn open(source: EditorSource, cx: &mut App) -> Task<Result<Self>> {
        let display = match source {
            EditorSource::File(path) => path,
            EditorSource::History(id) => {
                let file = cx
                    .try_global::<Services>()
                    .and_then(|s| s.history.read(cx).get(id).map(|entry| entry.file.clone()));
                match file {
                    Some(file) => file,
                    None => return Task::ready(Err(Error::NotFound(id.to_string()))),
                }
            }
            EditorSource::Recovery(journal) => return Self::recover(journal, cx),
            EditorSource::Staged(_) => return Task::ready(Err(Error::Unsupported)),
        };
        let loading = cx
            .background_executor()
            .spawn(async move { Ok::<_, Error>((load(&display)?, BundledText::bundled())) });
        cx.spawn(async move |cx| {
            let (opened, text) = loading.await?;
            Ok(cx.update(|cx| Self::new(opened, Arc::new(text), cx)))
        })
    }

    fn recover(journal: PathBuf, cx: &mut App) -> Task<Result<Self>> {
        let loading = cx.background_executor().spawn(async move {
            let entry = journal::load(&journal)
                .map_err(|e| Error::Protocol(e.to_string()))?
                .ok_or_else(|| Error::NotFound(journal.display().to_string()))?;
            let reopened = es_store::reopen(&entry.display);
            let mut opened = load(&entry.display)?;
            if entry.base_image != reopened.source {
                opened.base = image::open(&entry.base_image)
                    .map_err(|e| Error::Protocol(e.to_string()))?
                    .to_rgba8();
            }
            let baseline = reopened.document.unwrap_or_default();
            opened.document = entry.document;
            let session = journal::session_of(&journal).unwrap_or_default();
            Ok::<_, Error>((opened, baseline, session, BundledText::bundled()))
        });
        cx.spawn(async move |cx| {
            let (opened, baseline, session, text) = loading.await?;
            Ok(cx.update(|cx| {
                let this = Self::new(opened, Arc::new(text), cx);
                this.io
                    .update(cx, |io, _| io.adopt_recovery(session, baseline));
                this
            }))
        })
    }

    pub fn new(mut opened: Opened, text: Arc<BundledText>, cx: &mut App) -> Self {
        let services = cx.try_global::<Services>();
        let paths = services.map(|s| s.stores.paths.clone());
        let config = services.map(|s| s.settings.read(cx).config().clone());
        look::opening_look(&mut opened.document, paths.as_ref(), config.as_deref());
        let doc = cx.new(|_| DocModel::new(opened, text, &StylePreset::default()));
        let viewport = cx.new(|cx| ViewportModel::new(doc.clone(), cx));
        let io = cx.new(|cx| EditorIo::new(doc.clone(), cx));
        Self {
            look: cx.new(|cx| LookModel::new(doc.clone(), paths, cx)),
            crop: cx.new(|_| CropModel::new(doc.clone(), viewport.clone())),
            doc,
            io,
            viewport,
        }
    }

    /// Engine stack first, then the image stack; nothing while cropping.
    pub fn undo(&self, cx: &mut App) {
        if self.crop.read(cx).is_cropping() {
            return;
        }
        if self.doc.read(cx).editor().can_undo() {
            self.doc.update(cx, |d, cx| d.apply(cx, |e| e.undo()));
        } else {
            self.crop.update(cx, |c, cx| c.undo(cx));
        }
    }

    /// Engine stack first, then the image stack; nothing while cropping.
    pub fn redo(&self, cx: &mut App) {
        if self.crop.read(cx).is_cropping() {
            return;
        }
        if self.doc.read(cx).editor().can_redo() {
            self.doc.update(cx, |d, cx| d.apply(cx, |e| e.redo()));
        } else {
            self.crop.update(cx, |c, cx| c.redo(cx));
        }
    }

    pub fn can_undo(&self, cx: &App) -> bool {
        let crop = self.crop.read(cx);
        !crop.is_cropping() && (self.doc.read(cx).editor().can_undo() || crop.can_undo())
    }

    pub fn can_redo(&self, cx: &App) -> bool {
        let crop = self.crop.read(cx);
        !crop.is_cropping() && (self.doc.read(cx).editor().can_redo() || crop.can_redo(cx))
    }
}
