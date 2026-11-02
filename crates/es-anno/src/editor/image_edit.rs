use super::history::History;
use super::{Editor, Interaction};
use crate::Changes;
use crate::document::Snapshot;

/// The engine's undo history, held aside while an edit of the base image owns the shapes.
#[derive(Default)]
pub struct EditHistory(History);

impl EditHistory {
    /// A new edit makes what was undone before it unreachable.
    pub fn forget_redo(&mut self) {
        self.0.clear_redo();
    }
}

impl Editor {
    /// Changes with every new undo entry, so a caller can tell when its own redo went stale.
    pub fn history_revision(&self) -> u64 {
        self.history.revision()
    }

    /// Undo entries hold shapes in the old image's coordinates, so they leave with the old image.
    pub fn swap_document(
        &mut self,
        snapshot: Snapshot,
        history: EditHistory,
    ) -> (Changes, Snapshot, EditHistory) {
        let mut ch = self.stop_text();
        self.end_gesture(&mut ch);
        let before = self.doc.snapshot();
        let old = std::mem::replace(&mut self.history, history.0);
        self.doc.restore(snapshot);
        self.interaction = Interaction::default();
        self.hinted_binding_id = None;
        let kept: Vec<_> = self.selected_ids.iter().copied().collect();
        self.set_selection(kept, &mut ch);
        ch.all_dirty = true;
        ch.undo_state_changed = true;
        ch.needs_redraw = true;
        (ch, before, EditHistory(old))
    }
}
