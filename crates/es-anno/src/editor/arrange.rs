use super::Editor;
use crate::{Changes, ShapeId};
use es_geom::{BBox, Point};
use es_model::{AnnoShape, AnnoShapeId, ArrowBinding, Tool};
use std::collections::HashMap;

pub const COPY_OFFSET: f64 = 16.0;

pub(crate) struct Clipboard {
    shapes: Vec<AnnoShape>,
    bindings: Vec<ArrowBinding>,
}

#[derive(Clone, Copy)]
enum ZMove {
    Forward,
    Backward,
    Front,
    Back,
}

impl Editor {
    /// Bindings between copies are remapped; an arrow copied without its target is unbound.
    pub fn duplicate_selected(&mut self) -> Changes {
        if self.selected_ids.is_empty() {
            return Changes::default();
        }
        self.command(|ed, ch| {
            let clip = ed.clip_of(&ed.selected_in_order());
            let ids = ed.insert_copies(&clip, ch);
            ed.set_selection(ids, ch);
        })
    }

    pub fn has_clipboard(&self) -> bool {
        self.clipboard.is_some()
    }

    /// The in-app clipboard holds shapes only, and only from the select tool.
    pub fn copy_shapes(&mut self) -> Changes {
        if self.tool == Tool::Select && !self.selected_ids.is_empty() {
            self.clipboard = Some(self.clip_of(&self.selected_in_order()));
        }
        Changes::default()
    }

    /// Centred on `at` when given, else offset from the last copy or paste so pastes cascade.
    pub fn paste_shapes(&mut self, at: Option<Point>) -> Changes {
        let Some(clip) = self.clipboard.take() else {
            return Changes::default();
        };
        let at = at.filter(|p| p.x.is_finite() && p.y.is_finite());
        let ch = self.command(|ed, ch| {
            let ids = ed.insert_copies(&clip, ch);
            if let Some(p) = at
                && let Some(c) = ed.group_center(&ids)
            {
                let (dx, dy) = (p.x - c.x, p.y - c.y);
                ed.edit(ch, |doc| {
                    ids.iter()
                        .flat_map(|id| {
                            doc.update(*id, |s| {
                                s.x += dx;
                                s.y += dy;
                            })
                        })
                        .collect()
                });
            }
            ed.clipboard = Some(ed.clip_of(&ids));
            ed.set_selection(ids, ch);
        });
        if self.clipboard.is_none() {
            self.clipboard = Some(clip);
        }
        ch
    }

    pub fn bring_forward(&mut self) -> Changes {
        self.arrange(ZMove::Forward)
    }

    pub fn send_backward(&mut self) -> Changes {
        self.arrange(ZMove::Backward)
    }

    pub fn bring_to_front(&mut self) -> Changes {
        self.arrange(ZMove::Front)
    }

    pub fn send_to_back(&mut self) -> Changes {
        self.arrange(ZMove::Back)
    }

    /// Wraps around; with nothing selected it starts from the back.
    pub fn select_next(&mut self) -> Changes {
        self.cycle(true)
    }

    /// Wraps around; with nothing selected it starts from the front.
    pub fn select_prev(&mut self) -> Changes {
        self.cycle(false)
    }

    fn cycle(&mut self, forward: bool) -> Changes {
        let mut ch = Changes::default();
        let shapes = self.doc.shapes();
        let n = shapes.len();
        if n == 0 {
            return ch;
        }
        let positions = || {
            shapes
                .iter()
                .enumerate()
                .filter(|(_, s)| self.selected_ids.contains(&s.id))
                .map(|(i, _)| i)
        };
        let index = if forward {
            positions().max().map_or(0, |i| (i + 1) % n)
        } else {
            positions().min().map_or(n - 1, |i| (i + n - 1) % n)
        };
        let id = shapes[index].id;
        self.set_tool(Tool::Select, &mut ch);
        self.set_selection([id], &mut ch);
        self.sync_style_from_selection();
        ch
    }

    /// Redactions and spotlights keep their slots so their fixed draw order holds.
    fn arrange(&mut self, mv: ZMove) -> Changes {
        if self.selected_ids.is_empty() {
            return Changes::default();
        }
        let shapes = self.doc.shapes();
        let slots: Vec<usize> = (0..shapes.len())
            .filter(|&i| !shapes[i].is_redaction() && !shapes[i].is_highlight())
            .collect();
        let mut seq: Vec<AnnoShapeId> = slots.iter().map(|&i| shapes[i].id).collect();
        let sel = |id: &AnnoShapeId| self.selected_ids.contains(id);
        match mv {
            ZMove::Front => seq.sort_by_key(|id| sel(id)),
            ZMove::Back => seq.sort_by_key(|id| !sel(id)),
            ZMove::Forward => {
                for i in (0..seq.len().saturating_sub(1)).rev() {
                    if sel(&seq[i]) && !sel(&seq[i + 1]) {
                        seq.swap(i, i + 1);
                    }
                }
            }
            ZMove::Backward => {
                for i in 1..seq.len() {
                    if sel(&seq[i]) && !sel(&seq[i - 1]) {
                        seq.swap(i - 1, i);
                    }
                }
            }
        }
        let mut order: Vec<AnnoShapeId> = shapes.iter().map(|s| s.id).collect();
        for (&slot, id) in slots.iter().zip(seq) {
            order[slot] = id;
        }
        self.command(|ed, ch| {
            if ed.doc.reorder(&order) {
                ch.all_dirty = true;
                ed.record_change(ch);
            }
        })
    }

    fn clip_of(&self, ids: &[AnnoShapeId]) -> Clipboard {
        let shapes: Vec<AnnoShape> = ids
            .iter()
            .filter_map(|id| self.doc.shape(*id).cloned())
            .collect();
        let bindings = self
            .doc
            .bindings()
            .iter()
            .filter(|b| ids.contains(&b.arrow_id) && ids.contains(&b.to_id))
            .cloned()
            .collect();
        Clipboard { shapes, bindings }
    }

    fn insert_copies(&mut self, clip: &Clipboard, ch: &mut Changes) -> Vec<AnnoShapeId> {
        let mut renamed = HashMap::new();
        let copies: Vec<AnnoShape> = clip
            .shapes
            .iter()
            .map(|s| {
                let mut copy = s.clone();
                copy.id = AnnoShapeId::new();
                copy.x += COPY_OFFSET;
                copy.y += COPY_OFFSET;
                renamed.insert(s.id, copy.id);
                copy
            })
            .collect();
        let bindings: Vec<ArrowBinding> = clip
            .bindings
            .iter()
            .filter_map(|b| {
                Some(ArrowBinding {
                    arrow_id: *renamed.get(&b.arrow_id)?,
                    to_id: *renamed.get(&b.to_id)?,
                    ..b.clone()
                })
            })
            .collect();
        let ids = copies.iter().map(|c| c.id).collect();
        self.edit(ch, |doc| {
            let mut dirty: Vec<ShapeId> = Vec::new();
            for copy in copies {
                dirty.extend(doc.add(copy));
            }
            for b in bindings {
                dirty.extend(doc.set_binding(b));
            }
            dirty
        });
        ids
    }

    fn group_center(&self, ids: &[AnnoShapeId]) -> Option<Point> {
        let boxes: Vec<BBox> = ids
            .iter()
            .filter_map(|id| self.doc.page_bounds(*id))
            .collect();
        if boxes.is_empty() {
            return None;
        }
        let c = BBox::common(&boxes).center();
        Some(Point::new(c.x, c.y))
    }
}
