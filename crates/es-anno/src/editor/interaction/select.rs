use super::{Interaction, absorb};
use crate::editor::Editor;
use crate::{Changes, CursorKind, PointerInfo};
use es_geom::math::point_in_polygon;
use es_geom::{PVec, Point, Rect};
use es_model::AnnoShapeId;

impl Editor {
    /// A handle wins over a shape, a shape over the selection's frame, the frame over a marquee.
    pub(super) fn begin_select(
        &mut self,
        p: PVec,
        hit: Option<AnnoShapeId>,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        if let Some(handle) = self.handle_at(pointer.screen) {
            self.begin_handle(handle, p, ch);
            return;
        }
        if let Some(id) = hit {
            if pointer.click_count >= 2 && !pointer.shift && self.is_text_shape(id) {
                absorb(ch, self.begin_text(id));
                return;
            }
            self.select_and_translate(id, p, pointer.shift, ch);
            return;
        }
        if self.in_selection_frame(p) {
            self.start_translate(p, ch);
            return;
        }
        if !pointer.shift {
            self.set_selection([], ch);
        }
        self.interaction = Interaction::Brushing {
            origin: p,
            corner: p,
        };
        ch.needs_redraw = true;
    }

    pub(super) fn select_and_translate(
        &mut self,
        id: AnnoShapeId,
        p: PVec,
        shift: bool,
        ch: &mut Changes,
    ) {
        let mut next = self.selected_ids.clone();
        if shift {
            if !next.remove(&id) {
                next.insert(id);
            }
        } else if !next.contains(&id) {
            next = [id].into();
        }
        if next != self.selected_ids {
            self.set_selection(next, ch);
            self.sync_style_from_selection();
        }
        if !self.selected_ids.is_empty() {
            self.start_translate(p, ch);
        }
    }

    /// A lone arrow's frame is mostly empty space, so only its line grabs it.
    fn in_selection_frame(&self, p: PVec) -> bool {
        let ids = self.selected_in_order();
        if let [id] = ids.as_slice()
            && self.doc.shape(*id).is_none_or(|s| s.is_arrow())
        {
            return false;
        }
        self.selection_bounds()
            .is_some_and(|b| point_in_polygon(p, &b.page_corners()))
    }

    fn start_translate(&mut self, origin: PVec, ch: &mut Changes) {
        self.begin_fresh_gesture(ch);
        let initial = self
            .selected_in_order()
            .into_iter()
            .filter_map(|id| self.doc.shape(id).map(|s| (id, PVec::new(s.x, s.y))))
            .collect();
        self.interaction = Interaction::Translating { origin, initial };
    }

    /// Positions are set from where the drag began, so moves never accumulate error.
    pub(super) fn translate(
        &mut self,
        origin: PVec,
        initial: &[(AnnoShapeId, PVec)],
        p: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let (mut dx, mut dy) = (p.x - origin.x, p.y - origin.y);
        if pointer.shift {
            if dx.abs() > dy.abs() {
                dy = 0.0;
            } else {
                dx = 0.0;
            }
        }
        self.edit(ch, |doc| {
            initial
                .iter()
                .flat_map(|(id, start)| {
                    doc.update(*id, |s| {
                        s.x = start.x + dx;
                        s.y = start.y + dy;
                    })
                })
                .collect()
        });
    }

    /// Shapes the marquee touches are selected, not only those inside it; Shift adds to the selection.
    pub(super) fn finish_brush(
        &mut self,
        origin: PVec,
        corner: PVec,
        pointer: &PointerInfo,
        ch: &mut Changes,
    ) {
        let hits = self.shapes_in(Rect::from_points(
            Point::new(origin.x, origin.y),
            Point::new(corner.x, corner.y),
        ));
        let mut next = if pointer.shift {
            self.selected_ids.clone()
        } else {
            Default::default()
        };
        next.extend(hits);
        if next != self.selected_ids {
            self.set_selection(next, ch);
            self.sync_style_from_selection();
        }
    }

    pub(super) fn select_hover_cursor(
        &self,
        screen: Point,
        page: Point,
        hit: Option<AnnoShapeId>,
    ) -> CursorKind {
        if let Some(handle) = self.handle_at(screen) {
            return self.handle_cursor(handle);
        }
        if hit.is_some() || self.in_selection_frame(PVec::new(page.x, page.y)) {
            CursorKind::OpenHand
        } else {
            CursorKind::Arrow
        }
    }
}
