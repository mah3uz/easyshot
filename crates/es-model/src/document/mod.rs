use crate::background::BackgroundSettings;
use crate::shape::{AnnoShape, AnnoShapeId, ArrowBinding, ArrowTerminal, ShapeKind};
use serde::{Deserialize, Serialize};

/// The body of the `easyshot.document` sidecar. `shapes` run back to front (z-order).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnnotationDocument {
    pub base_image_file_name: String,
    pub shapes: Vec<AnnoShape>,
    pub bindings: Vec<ArrowBinding>,
    pub background: BackgroundSettings,
}

impl AnnotationDocument {
    pub fn shape(&self, id: AnnoShapeId) -> Option<&AnnoShape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    /// Adds on top of the z-order.
    pub fn add_shape(&mut self, shape: AnnoShape) {
        self.shapes.push(shape);
    }

    /// Also drops the bindings of the shape and every binding that targets it.
    pub fn remove_shape(&mut self, id: AnnoShapeId) -> Option<AnnoShape> {
        let index = self.shapes.iter().position(|s| s.id == id)?;
        self.bindings.retain(|b| b.arrow_id != id && b.to_id != id);
        Some(self.shapes.remove(index))
    }

    pub fn binding(&self, arrow: AnnoShapeId, terminal: ArrowTerminal) -> Option<&ArrowBinding> {
        self.bindings
            .iter()
            .find(|b| b.arrow_id == arrow && b.terminal == terminal)
    }

    /// At most one binding per arrow terminal: a new one replaces the old.
    pub fn set_binding(&mut self, binding: ArrowBinding) {
        self.remove_binding(binding.arrow_id, binding.terminal);
        self.bindings.push(binding);
    }

    pub fn remove_binding(&mut self, arrow: AnnoShapeId, terminal: ArrowTerminal) {
        self.bindings
            .retain(|b| !(b.arrow_id == arrow && b.terminal == terminal));
    }

    /// One more than the highest number in use, so deleting the top number reuses it and
    /// deleting a middle one leaves a gap.
    pub fn next_numbered_value(&self) -> u32 {
        self.shapes
            .iter()
            .filter_map(|s| match &s.kind {
                ShapeKind::Numbered(n) => Some(n.value),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    /// False when committing would write nothing: no shapes and nothing to render behind them.
    pub fn has_content(&self) -> bool {
        !self.shapes.is_empty() || self.background.has_renderable_content()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{BackgroundColor, BackgroundStyle};
    use crate::shape::{ArrowProps, GeoProps, NumberedProps};

    fn numbered(value: u32) -> AnnoShape {
        AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Numbered(NumberedProps {
                value,
                ..NumberedProps::default()
            }),
        )
    }

    fn bind(arrow: AnnoShapeId, to: AnnoShapeId, terminal: ArrowTerminal) -> ArrowBinding {
        ArrowBinding {
            arrow_id: arrow,
            to_id: to,
            terminal,
            ..ArrowBinding::default()
        }
    }

    #[test]
    fn deleting_a_shape_deletes_bindings_from_and_to_it() {
        let mut doc = AnnotationDocument::default();
        let rect = AnnoShape::new(0.0, 0.0, ShapeKind::Geo(GeoProps::default()));
        let other = AnnoShape::new(0.0, 0.0, ShapeKind::Geo(GeoProps::default()));
        let arrow = AnnoShape::new(0.0, 0.0, ShapeKind::Arrow(ArrowProps::default()));
        let (r, o, a) = (rect.id, other.id, arrow.id);
        for s in [rect, other, arrow] {
            doc.add_shape(s);
        }
        doc.set_binding(bind(a, r, ArrowTerminal::Start));
        doc.set_binding(bind(a, o, ArrowTerminal::End));
        doc.remove_shape(r);
        assert_eq!(doc.bindings.len(), 1);
        assert!(doc.binding(a, ArrowTerminal::Start).is_none());
        doc.remove_shape(a);
        assert!(doc.bindings.is_empty());
        assert!(doc.remove_shape(a).is_none());
    }

    #[test]
    fn a_terminal_holds_one_binding() {
        let mut doc = AnnotationDocument::default();
        let (a, x, y) = (AnnoShapeId::new(), AnnoShapeId::new(), AnnoShapeId::new());
        doc.set_binding(bind(a, x, ArrowTerminal::End));
        doc.set_binding(bind(a, y, ArrowTerminal::End));
        doc.set_binding(bind(a, x, ArrowTerminal::Start));
        assert_eq!(doc.bindings.len(), 2);
        assert_eq!(doc.binding(a, ArrowTerminal::End).map(|b| b.to_id), Some(y));
    }

    #[test]
    fn numbering_reuses_the_top_and_leaves_gaps() {
        let mut doc = AnnotationDocument::default();
        assert_eq!(doc.next_numbered_value(), 1);
        let ids: Vec<_> = (1..=3)
            .map(|v| {
                let s = numbered(v);
                let id = s.id;
                doc.add_shape(s);
                id
            })
            .collect();
        assert_eq!(doc.next_numbered_value(), 4);
        doc.remove_shape(ids[2]);
        assert_eq!(doc.next_numbered_value(), 3);
        doc.remove_shape(ids[0]);
        assert_eq!(doc.next_numbered_value(), 3);
    }

    #[test]
    fn z_order_is_insertion_order() {
        let mut doc = AnnotationDocument::default();
        let first = numbered(1);
        let second = numbered(2);
        let (f, s) = (first.id, second.id);
        doc.add_shape(first);
        doc.add_shape(second);
        let order: Vec<_> = doc.shapes.iter().map(|x| x.id).collect();
        assert_eq!(order, [f, s]);
    }

    #[test]
    fn an_empty_document_has_nothing_to_commit_until_a_look_is_added() {
        let mut doc = AnnotationDocument::default();
        assert!(!doc.has_content());
        doc.background.style = BackgroundStyle::Solid(BackgroundColor::custom(0.1, 0.1, 0.1));
        assert!(doc.has_content());
        let mut doc = AnnotationDocument::default();
        doc.add_shape(numbered(1));
        assert!(doc.has_content());
    }
}
