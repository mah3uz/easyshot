use es_anno::{BundledText, Changes, Editor, ShapeId, TextMeasure, Viewport};
use es_geom::Size;
use es_model::{AnnotationDocument, BackgroundSettings, StylePreset, TextProps};
use gpui::{Context, EventEmitter};
use image::RgbaImage;
use smallvec::SmallVec;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocEvent {
    ShapesChanged(SmallVec<[ShapeId; 4]>),
    AllChanged,
    SelectionChanged,
    ToolChanged,
    UndoChanged,
    TextEditing(Option<ShapeId>),
}

/// What an editor opens: the base image it renders from and the document drawn over it.
pub struct Opened {
    pub display: PathBuf,
    pub base: RgbaImage,
    pub document: AnnotationDocument,
    pub read_only: bool,
}

struct SharedText(Arc<BundledText>);

impl TextMeasure for SharedText {
    fn measure(&self, props: &TextProps) -> Size {
        self.0.measure(props)
    }
}

pub struct DocModel {
    editor: Editor,
    text: Arc<BundledText>,
    base: Arc<RgbaImage>,
    base_revision: u64,
    background: BackgroundSettings,
    display: PathBuf,
    read_only: bool,
}

impl EventEmitter<DocEvent> for DocModel {}

impl DocModel {
    pub fn new(opened: Opened, text: Arc<BundledText>, preset: &StylePreset) -> Self {
        let background = opened.document.background.clone();
        let editor = Editor::new(opened.document, Box::new(SharedText(text.clone())), preset);
        Self {
            editor,
            text,
            base: Arc::new(opened.base),
            base_revision: 1,
            background,
            display: opened.display,
            read_only: opened.read_only,
        }
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    pub fn text(&self) -> &Arc<BundledText> {
        &self.text
    }

    pub fn base(&self) -> &Arc<RgbaImage> {
        &self.base
    }

    /// Changes only when the base pixels are replaced, as on crop, rotate or flip.
    pub fn base_revision(&self) -> u64 {
        self.base_revision
    }

    pub fn image_size(&self) -> Size {
        Size::new(f64::from(self.base.width()), f64::from(self.base.height()))
    }

    /// The look the document was saved with.
    pub fn background(&self) -> &BackgroundSettings {
        &self.background
    }

    pub fn display(&self) -> &PathBuf {
        &self.display
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Runs one engine call and turns its `Changes` into events.
    pub fn apply(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Editor) -> Changes,
    ) -> Changes {
        let editing = self.editing_handle();
        let changes = f(&mut self.editor);
        let after = self.editing_handle();
        for event in events(&changes, (editing != after).then_some(after)) {
            cx.emit(event);
        }
        if changes != Changes::default() || editing != after {
            cx.notify();
        }
        changes
    }

    pub fn replace_base(&mut self, base: RgbaImage, cx: &mut Context<Self>) {
        self.base = Arc::new(base);
        self.base_revision += 1;
        cx.notify();
    }

    pub(crate) fn set_viewport(&mut self, viewport: Viewport) {
        let _ = self.editor.set_viewport(viewport);
    }

    fn editing_handle(&self) -> Option<ShapeId> {
        let doc = self.editor.doc();
        self.editor.editing_text_id().and_then(|id| doc.handle(id))
    }
}

fn events(ch: &Changes, editing: Option<Option<ShapeId>>) -> Vec<DocEvent> {
    let mut out = Vec::new();
    if ch.all_dirty {
        out.push(DocEvent::AllChanged);
    } else if !ch.dirty_shapes.is_empty() {
        out.push(DocEvent::ShapesChanged(ch.dirty_shapes.clone()));
    }
    if ch.selection_changed {
        out.push(DocEvent::SelectionChanged);
    }
    if ch.tool_changed {
        out.push(DocEvent::ToolChanged);
    }
    if ch.undo_state_changed {
        out.push(DocEvent::UndoChanged);
    }
    if let Some(id) = editing {
        out.push(DocEvent::TextEditing(id));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_document_changes_are_not_also_reported_per_shape() {
        let mut ch = Changes {
            all_dirty: true,
            undo_state_changed: true,
            ..Changes::default()
        };
        ch.dirty_shapes.push(ShapeId(3));
        assert_eq!(
            events(&ch, None),
            vec![DocEvent::AllChanged, DocEvent::UndoChanged]
        );
        assert_eq!(
            events(&Changes::default(), Some(None)),
            vec![DocEvent::TextEditing(None)]
        );
    }
}
