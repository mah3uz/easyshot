use super::history::GestureKey;
use super::{Editor, GestureId, Interaction};
use crate::Changes;
use crate::style::StylePatch;
use es_model::{AnnoShape, AnnoShapeId, ShapeKind, Tool};

impl Editor {
    /// Picking the active tool again returns to the select tool; the highlighter switches mode instead.
    pub fn select_tool(&mut self, tool: Tool) -> Changes {
        let mut ch = Changes::default();
        if tool == Tool::Highlighter && self.tool == Tool::Highlighter {
            self.highlighter_straight = !self.highlighter_straight;
            ch.tool_changed = true;
            return ch;
        }
        let next = if tool == self.tool {
            Tool::Select
        } else {
            tool
        };
        self.set_tool(next, &mut ch);
        ch
    }

    /// Ids not in the document are ignored.
    pub fn select(&mut self, ids: impl IntoIterator<Item = AnnoShapeId>) -> Changes {
        let mut ch = Changes::default();
        self.set_selection(ids, &mut ch);
        self.sync_style_from_selection();
        ch
    }

    /// One undo entry for the whole batch; the new shapes end up selected.
    pub fn add_shapes(&mut self, shapes: Vec<AnnoShape>) -> Changes {
        if shapes.is_empty() {
            return Changes::default();
        }
        self.command(|ed, ch| {
            ed.set_tool(Tool::Select, ch);
            let ids: Vec<_> = shapes.iter().map(|s| s.id).collect();
            ed.edit(ch, |doc| {
                shapes.into_iter().flat_map(|s| doc.add(s)).collect()
            });
            ed.set_selection(ids, ch);
        })
    }

    pub fn select_all(&mut self) -> Changes {
        let mut ch = Changes::default();
        self.set_tool(Tool::Select, &mut ch);
        let all: Vec<_> = self.doc.shapes().iter().map(|s| s.id).collect();
        self.set_selection(all, &mut ch);
        ch
    }

    /// Ids no longer in the document are ignored.
    pub fn delete_shapes(&mut self, ids: &[AnnoShapeId]) -> Changes {
        let present: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| self.doc.shape(*id).is_some())
            .collect();
        if present.is_empty() {
            return Changes::default();
        }
        self.command(|ed, ch| {
            ed.edit(ch, |doc| doc.delete(&present));
            let kept: Vec<_> = ed
                .selected_ids
                .iter()
                .copied()
                .filter(|id| !present.contains(id))
                .collect();
            ed.set_selection(kept, ch);
        })
    }

    pub fn delete_selected(&mut self) -> Changes {
        if self.selected_ids.is_empty() {
            return Changes::default();
        }
        self.command(|ed, ch| {
            let ids = ed.selected_in_order();
            ed.edit(ch, |doc| doc.delete(&ids));
            ed.set_selection([], ch);
            ed.stop_text_into(ch);
        })
    }

    /// Calls sharing `gesture`, the repeats of one held key, make one undo entry.
    pub fn nudge_selected(&mut self, dx: f64, dy: f64, gesture: GestureId) -> Changes {
        let mut ch = Changes::default();
        let ids = self.selected_in_order();
        if ids.is_empty() || !dx.is_finite() || !dy.is_finite() {
            return ch;
        }
        self.begin_gesture(GestureKey::Nudge(gesture), &mut ch);
        self.edit(&mut ch, |doc| {
            ids.iter()
                .flat_map(|id| {
                    doc.update(*id, |s| {
                        s.x += dx;
                        s.y += dy;
                    })
                })
                .collect()
        });
        ch
    }

    /// Calls sharing `gesture` make one undo entry, or join the text session being edited.
    pub fn apply_style(&mut self, patch: StylePatch, gesture: GestureId) -> Changes {
        let mut ch = Changes::default();
        let Some(patch) = patch.normalized() else {
            return ch;
        };
        patch.apply_to_current(&mut self.current);
        let targets = if patch.is_text() {
            self.single_selected_text().into_iter().collect()
        } else {
            self.selected_in_order()
        };
        if targets.is_empty() {
            return ch;
        }
        let key = match self.editing_text_id {
            Some(id) => GestureKey::Text(id),
            None => GestureKey::Style(gesture),
        };
        self.begin_gesture(key, &mut ch);
        let page_stroke = self.viewport.page_stroke_width(self.current.stroke_width);
        self.edit(&mut ch, |doc| {
            targets
                .iter()
                .flat_map(|id| doc.update(*id, |s| patch.apply_to_shape(s, page_stroke)))
                .collect()
        });
        ch
    }

    pub fn undo(&mut self) -> Changes {
        self.step(true)
    }

    pub fn redo(&mut self) -> Changes {
        self.step(false)
    }

    fn step(&mut self, back: bool) -> Changes {
        let mut ch = Changes::default();
        self.end_gesture(&mut ch);
        let snapshot = if back {
            self.history.undo(&self.doc)
        } else {
            self.history.redo(&self.doc)
        };
        let Some(snapshot) = snapshot else {
            return ch;
        };
        self.doc.restore(snapshot);
        ch.all_dirty = true;
        ch.undo_state_changed = true;
        ch.needs_redraw = true;
        self.interaction = Interaction::default();
        self.hinted_binding_id = None;
        if let Some(id) = self.editing_text_id.take()
            && self.is_blank_text(id)
        {
            self.doc.delete(&[id]);
        }
        let kept: Vec<_> = self.selected_ids.iter().copied().collect();
        self.set_selection(kept, &mut ch);
        ch
    }

    pub(super) fn single_selected_text(&self) -> Option<AnnoShapeId> {
        if self.selected_ids.len() != 1 {
            return None;
        }
        let id = *self.selected_ids.first()?;
        self.doc.shape(id)?.is_text().then_some(id)
    }

    pub(crate) fn is_blank_text(&self, id: AnnoShapeId) -> bool {
        matches!(
            self.doc.shape(id).map(|s| &s.kind),
            Some(ShapeKind::Text(p)) if p.text.trim().is_empty()
        )
    }
}
