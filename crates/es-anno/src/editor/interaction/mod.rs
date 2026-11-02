use super::Editor;
use crate::{Changes, CursorKind, PointerInfo};
use es_geom::Point;

#[derive(Debug, Default)]
pub enum Interaction {
    #[default]
    Idle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Rotate { corner: u8 },
    ArrowStart,
    ArrowMiddle,
    ArrowEnd,
}

impl Editor {
    pub fn pointer_down(&mut self, _pointer: PointerInfo) -> Changes {
        todo!()
    }

    pub fn pointer_move(&mut self, _pointer: PointerInfo) -> Changes {
        todo!()
    }

    pub fn pointer_up(&mut self, _pointer: PointerInfo) -> Changes {
        todo!()
    }

    pub fn handle_at(&self, _screen: Point) -> Option<Handle> {
        todo!()
    }

    pub fn cursor_for(&self, _screen: Point) -> CursorKind {
        todo!()
    }
}
