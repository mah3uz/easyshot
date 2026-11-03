use crate::Changes;
use crate::document::Document;
use crate::editor::Editor;
use es_geom::{BBox, PVec};
use es_model::AnnoShapeId;

const EDGE_EPSILON: f64 = 1e-6;

pub(super) fn clamp_span(d: f64, lo: f64, hi: f64) -> f64 {
    d.clamp(lo.min(hi), lo.max(hi))
}

pub(super) fn all_within(
    doc: &Document,
    image: BBox,
    ids: impl IntoIterator<Item = AnnoShapeId>,
) -> bool {
    let room = image.expand_by(EDGE_EPSILON);
    ids.into_iter()
        .all(|id| doc.page_bounds(id).is_none_or(|b| room.contains(b)))
}

impl Editor {
    /// None while the image has no size, so an editor without one stays unbounded.
    pub(super) fn image_rect(&self) -> Option<BBox> {
        let size = self.viewport.image_size;
        (size.width > 0.0 && size.height > 0.0)
            .then(|| BBox::new(0.0, 0.0, size.width, size.height))
    }

    pub(super) fn on_image(&self, p: PVec) -> bool {
        self.image_rect().is_none_or(|r| r.contains_point(p, 0.0))
    }

    pub(super) fn clamp_to_image(&self, p: PVec) -> PVec {
        match self.image_rect() {
            Some(r) => PVec::new(
                p.x.clamp(r.min_x(), r.max_x()),
                p.y.clamp(r.min_y(), r.max_y()),
            ),
            None => p,
        }
    }

    pub(super) fn bounds_of(&self, ids: impl IntoIterator<Item = AnnoShapeId>) -> Option<BBox> {
        let boxes: Vec<BBox> = ids
            .into_iter()
            .filter_map(|id| self.doc.page_bounds(id))
            .collect();
        (!boxes.is_empty()).then(|| BBox::common(&boxes))
    }

    /// Moves a shape that sticks out of the image back in without resizing it.
    pub(super) fn nudge_inside(&mut self, id: AnnoShapeId, ch: &mut Changes) {
        let (Some(image), Some(b)) = (self.image_rect(), self.doc.page_bounds(id)) else {
            return;
        };
        let dx = clamp_span(0.0, -b.min_x(), image.max_x() - b.max_x());
        let dy = clamp_span(0.0, -b.min_y(), image.max_y() - b.max_y());
        if dx != 0.0 || dy != 0.0 {
            self.edit(ch, |doc| {
                doc.update(id, |s| {
                    s.x += dx;
                    s.y += dy;
                })
            });
        }
    }
}
