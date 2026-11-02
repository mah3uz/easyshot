use super::Editor;
use crate::document::page_transform;
use es_geom::{BBox, Mat, PVec, Point, Rect};
use es_model::AnnoShapeId;

/// A lone shape keeps its own rotated frame; several shapes share an axis-aligned page frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionBounds {
    pub bbox: BBox,
    pub transform: Mat,
}

impl SelectionBounds {
    pub fn page_corners(&self) -> [PVec; 4] {
        self.bbox
            .corners()
            .map(|c| self.transform.apply_to_point(c))
    }

    pub fn page_center(&self) -> PVec {
        self.transform.apply_to_point(self.bbox.center())
    }

    pub fn page_point(&self, normalized: PVec) -> PVec {
        let b = self.bbox;
        self.transform.apply_to_point(PVec::new(
            b.x + b.w * normalized.x,
            b.y + b.h * normalized.y,
        ))
    }
}

impl Editor {
    /// At least 4 image pixels and 8 screen pixels, so thin strokes stay clickable when zoomed out.
    pub fn hit_margin(&self) -> f64 {
        self.viewport.page_distance(8.0).max(4.0)
    }

    pub fn hit_shape(&self, page: Point) -> Option<AnnoShapeId> {
        self.doc
            .hit_test(PVec::new(page.x, page.y), self.hit_margin())
    }

    pub fn shapes_in(&self, page: Rect) -> Vec<AnnoShapeId> {
        let r = page.abs();
        self.doc
            .overlapping(BBox::new(r.x0, r.y0, r.width(), r.height()))
    }

    pub fn selection_bounds(&self) -> Option<SelectionBounds> {
        let ids = self.selected_in_order();
        if let [id] = ids.as_slice() {
            let shape = self.doc.shape(*id)?;
            return Some(SelectionBounds {
                bbox: self.doc.geometry(*id)?.bounds(),
                transform: page_transform(shape),
            });
        }
        let boxes: Vec<BBox> = ids
            .iter()
            .filter_map(|id| self.doc.page_bounds(*id))
            .collect();
        if boxes.is_empty() {
            return None;
        }
        Some(SelectionBounds {
            bbox: BBox::common(&boxes),
            transform: Mat::IDENTITY,
        })
    }
}
