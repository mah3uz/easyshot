mod arrange;
mod commands;
mod history;
mod image_edit;
pub mod interaction;
mod selection;
mod text;
mod viewport;

pub use arrange::COPY_OFFSET;
pub use history::{GestureId, UNDO_LIMIT};
pub use image_edit::EditHistory;
pub use interaction::{Handle, Interaction};
pub use selection::SelectionBounds;
pub use viewport::Viewport;

use crate::document::{Document, TextMeasure};
use crate::style::CurrentStyle;
use crate::{Changes, ShapeId};
use arrange::Clipboard;
use es_model::{
    AnnoShapeId, AnnotationDocument, HIGHLIGHTER_WIDTH_FACTOR, ShapeKind, StylePreset, Tool,
};
use history::{GestureKey, History};
use std::collections::BTreeSet;

/// Document changes go through `edit` inside an open gesture; that is what makes them undoable.
pub struct Editor {
    doc: Document,
    selected_ids: BTreeSet<AnnoShapeId>,
    tool: Tool,
    interaction: Interaction,
    history: History,
    viewport: Viewport,
    current: CurrentStyle,
    highlighter_straight: bool,
    editing_text_id: Option<AnnoShapeId>,
    hinted_binding_id: Option<AnnoShapeId>,
    clipboard: Option<Clipboard>,
}

impl Changes {
    pub(crate) fn mark(&mut self, ids: impl IntoIterator<Item = ShapeId>) {
        for id in ids {
            if !self.dirty_shapes.contains(&id) {
                self.dirty_shapes.push(id);
            }
        }
        self.needs_redraw = true;
    }
}

impl Editor {
    pub fn new(
        model: AnnotationDocument,
        measure: Box<dyn TextMeasure>,
        preset: &StylePreset,
    ) -> Self {
        Self {
            doc: Document::new(model, measure),
            selected_ids: BTreeSet::new(),
            tool: preset.tool,
            interaction: Interaction::default(),
            history: History::default(),
            viewport: Viewport::default(),
            current: CurrentStyle::from(preset),
            highlighter_straight: preset.highlighter_straight,
            editing_text_id: None,
            hinted_binding_id: None,
            clipboard: None,
        }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    pub fn selected_ids(&self) -> &BTreeSet<AnnoShapeId> {
        &self.selected_ids
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn current_style(&self) -> &CurrentStyle {
        &self.current
    }

    pub fn preset(&self) -> StylePreset {
        StylePreset {
            highlighter_straight: self.highlighter_straight,
            ..self.current.to_preset(self.tool)
        }
    }

    pub fn highlighter_straight(&self) -> bool {
        self.highlighter_straight
    }

    pub fn editing_text_id(&self) -> Option<AnnoShapeId> {
        self.editing_text_id
    }

    pub fn hinted_binding_id(&self) -> Option<AnnoShapeId> {
        self.hinted_binding_id
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.interaction, Interaction::Idle)
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn set_viewport(&mut self, viewport: Viewport) -> Changes {
        let mut ch = Changes::default();
        if viewport != self.viewport {
            self.viewport = viewport;
            ch.needs_redraw = true;
        }
        ch
    }

    pub(crate) fn begin_gesture(&mut self, key: GestureKey, ch: &mut Changes) {
        if self.history.begin(key, &self.doc) {
            ch.undo_state_changed = true;
        }
    }

    pub(crate) fn begin_fresh_gesture(&mut self, ch: &mut Changes) {
        let key = self.history.fresh_key();
        self.begin_gesture(key, ch);
    }

    pub(crate) fn end_gesture(&mut self, ch: &mut Changes) {
        if self.history.end(&self.doc) {
            ch.undo_state_changed = true;
        }
    }

    pub(crate) fn edit(
        &mut self,
        ch: &mut Changes,
        f: impl FnOnce(&mut Document) -> Vec<ShapeId>,
    ) -> bool {
        if !self.history.has_open() {
            self.begin_fresh_gesture(ch);
        }
        let dirty = f(&mut self.doc);
        if dirty.is_empty() {
            return false;
        }
        ch.mark(dirty);
        self.record_change(ch);
        true
    }

    pub(crate) fn record_change(&mut self, ch: &mut Changes) {
        if self.history.record() {
            ch.undo_state_changed = true;
        }
        ch.needs_redraw = true;
    }

    pub(crate) fn command(&mut self, f: impl FnOnce(&mut Self, &mut Changes)) -> Changes {
        let mut ch = Changes::default();
        self.begin_fresh_gesture(&mut ch);
        f(self, &mut ch);
        self.end_gesture(&mut ch);
        ch
    }

    pub(crate) fn set_selection(
        &mut self,
        ids: impl IntoIterator<Item = AnnoShapeId>,
        ch: &mut Changes,
    ) {
        let next: BTreeSet<_> = ids
            .into_iter()
            .filter(|id| self.doc.shape(*id).is_some())
            .collect();
        if next != self.selected_ids {
            self.selected_ids = next;
            ch.selection_changed = true;
            ch.needs_redraw = true;
        }
    }

    pub(crate) fn selected_in_order(&self) -> Vec<AnnoShapeId> {
        self.doc
            .shapes()
            .iter()
            .filter(|s| self.selected_ids.contains(&s.id))
            .map(|s| s.id)
            .collect()
    }

    /// Leaving a tool commits text editing; only the select tool keeps a selection.
    pub(crate) fn set_tool(&mut self, tool: Tool, ch: &mut Changes) {
        if tool == self.tool {
            return;
        }
        self.stop_text_into(ch);
        self.tool = tool;
        ch.tool_changed = true;
        if tool != Tool::Select {
            self.set_selection([], ch);
        }
    }

    /// Selecting one shape shows its style in the inspector and makes it the next shape's style.
    pub(crate) fn sync_style_from_selection(&mut self) {
        if self.selected_ids.len() != 1 {
            return;
        }
        let Some(shape) = self.selected_ids.first().and_then(|id| self.doc.shape(*id)) else {
            return;
        };
        let tool = shape.tool();
        if tool.supports_color()
            && let Some(swatch) = shape.swatch()
        {
            self.current.swatch = swatch.clone();
        }
        if tool.supports_stroke_width() && shape.stroke_width() > 0.0 {
            let width = match shape.kind {
                ShapeKind::Highlighter(_) => shape.stroke_width() / HIGHLIGHTER_WIDTH_FACTOR,
                _ => shape.stroke_width(),
            };
            self.current.stroke_width = self.viewport.slider_stroke_width(width);
        }
        match &shape.kind {
            ShapeKind::Redaction(p) => self.current.redaction_density = p.density,
            ShapeKind::Text(p) => {
                self.current.font_family = p.font_family;
                self.current.font_size = p.font_size;
                self.current.bold = p.is_bold;
                self.current.italic = p.is_italic;
                self.current.underline = p.is_underline;
                self.current.align = p.align;
                self.current.text_fill = p.fill.clone();
                self.current.text_border = p.border.clone();
            }
            ShapeKind::Arrow(p) => {
                if tool == Tool::Arrow {
                    self.current.arrowhead_start = p.arrowhead_start;
                    self.current.arrowhead_end = p.arrowhead_end;
                }
                self.current.dash = p.dash;
            }
            _ => {}
        }
    }
}
