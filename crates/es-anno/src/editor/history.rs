use crate::document::{Document, Snapshot};
use es_model::AnnoShapeId;
use std::collections::VecDeque;

pub const UNDO_LIMIT: usize = 200;

/// One slider drag or one held key; calls that share it make one undo entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GestureId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GestureKey {
    Serial(u64),
    Style(GestureId),
    Nudge(GestureId),
    Text(AnnoShapeId),
}

struct Open {
    key: GestureKey,
    before: Option<Snapshot>,
}

/// Pushes on a gesture's first real change, so a gesture that changes nothing leaves no entry.
#[derive(Default)]
pub(crate) struct History {
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    open: Option<Open>,
    serial: u64,
}

impl History {
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn is_open(&self, key: GestureKey) -> bool {
        self.open.as_ref().is_some_and(|o| o.key == key)
    }

    pub(crate) fn has_open(&self) -> bool {
        self.open.is_some()
    }

    pub(crate) fn fresh_key(&mut self) -> GestureKey {
        self.serial += 1;
        GestureKey::Serial(self.serial)
    }

    pub(crate) fn begin(&mut self, key: GestureKey, doc: &Document) -> bool {
        if self.is_open(key) {
            return false;
        }
        let changed = self.end(doc);
        self.open = Some(Open {
            key,
            before: Some(doc.snapshot()),
        });
        changed
    }

    pub(crate) fn record(&mut self) -> bool {
        let Some(before) = self.open.as_mut().and_then(|o| o.before.take()) else {
            return false;
        };
        self.undo.push_back(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
        true
    }

    /// A gesture whose changes cancelled out takes its entry back.
    pub(crate) fn end(&mut self, doc: &Document) -> bool {
        let Some(open) = self.open.take() else {
            return false;
        };
        if open.before.is_none() && self.undo.back().is_some_and(|s| doc.matches(s)) {
            self.undo.pop_back();
            return true;
        }
        false
    }

    pub(crate) fn undo(&mut self, doc: &Document) -> Option<Snapshot> {
        let snapshot = self.undo.pop_back()?;
        self.redo.push(doc.snapshot());
        Some(snapshot)
    }

    pub(crate) fn redo(&mut self, doc: &Document) -> Option<Snapshot> {
        let snapshot = self.redo.pop()?;
        self.undo.push_back(doc.snapshot());
        Some(snapshot)
    }
}
