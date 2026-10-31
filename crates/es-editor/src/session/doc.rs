use es_anno::ShapeId;
use smallvec::SmallVec;

pub struct DocModel {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocEvent {
    ShapesChanged(SmallVec<[ShapeId; 4]>),
    AllChanged,
    SelectionChanged,
    ToolChanged,
    UndoChanged,
    TextEditing(Option<ShapeId>),
}
