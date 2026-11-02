use super::Handle;
use crate::document::page_transform;
use crate::editor::Editor;
use crate::{Changes, PointerInfo};
use es_geom::PVec;
use es_geom::arrow::{bend_for_middle, snap_bend};
use es_model::{AnnoShape, AnnoShapeId, ArrowBinding, ArrowTerminal, NormalizedAnchor, ShapeKind};

fn fraction(v: f64, origin: f64, size: f64) -> f64 {
    if size == 0.0 {
        0.5
    } else {
        (v - origin) / size
    }
}

impl Editor {
    pub(super) fn lone_arrow(&self) -> Option<&AnnoShape> {
        let ids = self.selected_in_order();
        let [id] = ids.as_slice() else {
            return None;
        };
        self.doc.shape(*id).filter(|s| s.is_arrow())
    }

    /// Arrows, spotlights and redactions are not targets; a closed shape catches anywhere inside.
    fn binding_target(
        &self,
        arrow: AnnoShapeId,
        page: PVec,
    ) -> Option<(AnnoShapeId, NormalizedAnchor)> {
        let margin = self.hit_margin();
        self.doc.shapes().iter().rev().find_map(|shape| {
            if shape.id == arrow || shape.is_arrow() || shape.is_highlight() || shape.is_redaction()
            {
                return None;
            }
            let geometry = self.doc.geometry(shape.id)?;
            let local = page_transform(shape).inverse().apply_to_point(page);
            if !geometry.hit_test_point(local, margin, true) {
                return None;
            }
            let b = geometry.bounds();
            let anchor = NormalizedAnchor {
                x: fraction(local.x, b.x, b.w),
                y: fraction(local.y, b.y, b.h),
            };
            Some((shape.id, anchor))
        })
    }

    /// Precise pins the terminal to the point under the pointer instead of the shape's centre.
    fn bind_terminal(
        &mut self,
        arrow: AnnoShapeId,
        terminal: ArrowTerminal,
        page: PVec,
        precise: bool,
        ch: &mut Changes,
    ) {
        let target = self.binding_target(arrow, page);
        match target {
            Some((to_id, normalized_anchor)) => self.edit(ch, |doc| {
                doc.set_binding(ArrowBinding {
                    arrow_id: arrow,
                    to_id,
                    terminal,
                    normalized_anchor,
                    is_precise: precise,
                    is_exact: false,
                })
            }),
            None => self.edit(ch, |doc| doc.remove_binding(arrow, terminal)),
        };
        let hint = target.map(|(id, _)| id);
        if self.hinted_binding_id != hint {
            self.hinted_binding_id = hint;
            ch.needs_redraw = true;
        }
    }

    /// Binds before placing the terminal: a binding still in place would pull it back on unbind.
    pub(super) fn place_terminal(
        &mut self,
        id: AnnoShapeId,
        terminal: ArrowTerminal,
        local: PVec,
        page: PVec,
        precise: bool,
        ch: &mut Changes,
    ) {
        self.bind_terminal(id, terminal, page, precise, ch);
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Arrow(a) = &mut s.kind {
                    match terminal {
                        ArrowTerminal::Start => a.start = local,
                        ArrowTerminal::End => a.end = local,
                    }
                }
            })
        });
    }

    /// Alt pins a dragged terminal precisely; the bend handle snaps back to straight near the chord.
    pub(super) fn drag_arrow_handle(
        &mut self,
        id: AnnoShapeId,
        handle: Handle,
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let Some(shape) = self.doc.shape(id) else {
            return;
        };
        let local = page_transform(shape).inverse().apply_to_point(p);
        let terminal = match handle {
            Handle::ArrowStart => ArrowTerminal::Start,
            Handle::ArrowEnd => ArrowTerminal::End,
            Handle::ArrowMiddle => return self.bend_arrow(id, local, ch),
            _ => return,
        };
        let local = PVec::new(local.x, local.y);
        self.place_terminal(id, terminal, local, p, pointer.alt, ch);
    }

    fn bend_arrow(&mut self, id: AnnoShapeId, local: PVec, ch: &mut Changes) {
        let Some(info) = self.doc.arrow_info(id) else {
            return;
        };
        let (start, end) = (info.start.handle, info.end.handle);
        if PVec::equals(start, end) {
            return;
        }
        let bend = snap_bend(bend_for_middle(start, end, local));
        self.edit(ch, |doc| {
            doc.update(id, |s| {
                if let ShapeKind::Arrow(a) = &mut s.kind {
                    a.bend = bend;
                }
            })
        });
    }
}
