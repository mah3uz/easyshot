mod arrow;
mod clamp;
mod create;
mod cursor;
mod handles;
mod select;

use super::{Editor, SelectionBounds};
use crate::{Changes, CursorKind, PointerButton, PointerInfo};
use es_geom::freehand::LiveStroke;
use es_geom::{BBox, PVec, Point, Rect};
use es_model::{AnnoShape, AnnoShapeId, Tool};

#[derive(Debug, Default)]
pub enum Interaction {
    #[default]
    Idle,
    CreatingBox {
        id: AnnoShapeId,
        origin: PVec,
    },
    CreatingArrow {
        id: AnnoShapeId,
        origin: PVec,
        last: PVec,
    },
    Drawing {
        id: AnnoShapeId,
        origin: PVec,
        stroke: Box<LiveStroke>,
    },
    Translating {
        origin: PVec,
        initial: Vec<(AnnoShapeId, PVec)>,
        bounds: BBox,
    },
    Brushing {
        origin: PVec,
        corner: PVec,
    },
    Resizing {
        handle: Handle,
        bounds: SelectionBounds,
        initial: Vec<AnnoShape>,
        cursor: CursorKind,
    },
    Rotating {
        center: PVec,
        start_angle: f64,
        initial: Vec<AnnoShape>,
    },
    DraggingArrowHandle {
        id: AnnoShapeId,
        handle: Handle,
        cursor: CursorKind,
    },
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

fn page_of(pointer: &PointerInfo) -> Option<PVec> {
    let p = PVec::new(pointer.page.x, pointer.page.y);
    p.is_finite().then_some(p)
}

fn absorb(ch: &mut Changes, other: Changes) {
    if !other.dirty_shapes.is_empty() {
        ch.mark(other.dirty_shapes);
    }
    ch.all_dirty |= other.all_dirty;
    ch.selection_changed |= other.selection_changed;
    ch.tool_changed |= other.tool_changed;
    ch.undo_state_changed |= other.undo_state_changed;
    ch.needs_redraw |= other.needs_redraw;
}

impl Editor {
    /// Only the primary button edits; a press on the text being typed belongs to the text field.
    pub fn pointer_down(&mut self, pointer: PointerInfo) -> Changes {
        let mut ch = Changes::default();
        if pointer.button != PointerButton::Primary {
            return ch;
        }
        let Some(p) = page_of(&pointer) else {
            return ch;
        };
        if !self.is_idle() {
            absorb(&mut ch, self.pointer_up(pointer));
        }
        let hit = self.hit_shape(pointer.page);
        if self.editing_text_id.is_some() && hit == self.editing_text_id {
            return ch;
        }
        self.stop_text_into(&mut ch);
        if !matches!(self.tool, Tool::Select | Tool::Text) && !self.on_image(p) {
            return ch;
        }
        match self.tool {
            Tool::Select => self.begin_select(p, hit, &pointer, &mut ch),
            Tool::Text => {
                let started = match hit.filter(|id| self.is_text_shape(*id)) {
                    Some(id) => self.begin_text(id),
                    None if self.on_image(p) => self.create_text(pointer.page),
                    None => return ch,
                };
                absorb(&mut ch, started);
                if let Some(id) = self.editing_text_id {
                    self.nudge_inside(id, &mut ch);
                }
            }
            Tool::NumberedCircle => self.create_numbered(p, &mut ch),
            Tool::Freehand => self.begin_drawing(p, &pointer, &mut ch),
            Tool::Line | Tool::Arrow => self.begin_arrow(p, &pointer, &mut ch),
            Tool::Rectangle
            | Tool::FilledRectangle
            | Tool::Ellipse
            | Tool::Highlight
            | Tool::Blur
            | Tool::Pixelate => self.begin_box(p, &mut ch),
        }
        ch
    }

    /// Moves the shape under the pointer whatever the tool; a press that hits no shape does nothing.
    pub fn pointer_down_move(&mut self, pointer: PointerInfo) -> Changes {
        let mut ch = Changes::default();
        if pointer.button != PointerButton::Primary {
            return ch;
        }
        let Some(p) = page_of(&pointer) else {
            return ch;
        };
        let Some(id) = self.hit_shape(pointer.page) else {
            return ch;
        };
        if !self.is_idle() {
            absorb(&mut ch, self.pointer_up(pointer));
        }
        self.stop_text_into(&mut ch);
        self.select_and_translate(id, p, pointer.shift, &mut ch);
        ch
    }

    pub fn pointer_move(&mut self, pointer: PointerInfo) -> Changes {
        let mut ch = Changes::default();
        let Some(p) = page_of(&pointer) else {
            return ch;
        };
        match std::mem::take(&mut self.interaction) {
            Interaction::Idle => {}
            Interaction::CreatingBox { id, origin } => {
                let p = self.clamp_to_image(p);
                self.resize_new_box(id, origin, p, &pointer, &mut ch);
                self.interaction = Interaction::CreatingBox { id, origin };
            }
            Interaction::CreatingArrow { id, origin, .. } => {
                let p = self.clamp_to_image(p);
                self.move_new_arrow_end(id, p, &pointer, &mut ch);
                self.interaction = Interaction::CreatingArrow {
                    id,
                    origin,
                    last: p,
                };
            }
            Interaction::Drawing {
                id,
                origin,
                mut stroke,
            } => {
                let p = self.clamp_to_image(p);
                self.extend_drawing(id, origin, &mut stroke, p, &pointer, &mut ch);
                self.interaction = Interaction::Drawing { id, origin, stroke };
            }
            Interaction::Translating {
                origin,
                initial,
                bounds,
            } => {
                self.translate(origin, &initial, bounds, p, &pointer, &mut ch);
                self.interaction = Interaction::Translating {
                    origin,
                    initial,
                    bounds,
                };
            }
            Interaction::Brushing { origin, .. } => {
                ch.needs_redraw = true;
                self.interaction = Interaction::Brushing { origin, corner: p };
            }
            Interaction::Resizing {
                handle,
                bounds,
                initial,
                cursor,
            } => {
                self.resize(handle, &bounds, &initial, p, &pointer, &mut ch);
                self.interaction = Interaction::Resizing {
                    handle,
                    bounds,
                    initial,
                    cursor,
                };
            }
            Interaction::Rotating {
                center,
                start_angle,
                initial,
            } => {
                self.rotate(center, start_angle, &initial, p, &pointer, &mut ch);
                self.interaction = Interaction::Rotating {
                    center,
                    start_angle,
                    initial,
                };
            }
            Interaction::DraggingArrowHandle { id, handle, cursor } => {
                let p = self.clamp_to_image(p);
                self.drag_arrow_handle(id, handle, p, &pointer, &mut ch);
                self.interaction = Interaction::DraggingArrowHandle { id, handle, cursor };
            }
        }
        ch
    }

    pub fn pointer_up(&mut self, pointer: PointerInfo) -> Changes {
        let mut ch = Changes::default();
        let p = page_of(&pointer);
        match std::mem::take(&mut self.interaction) {
            // An idle release must not end the text session its press just opened.
            Interaction::Idle => return ch,
            Interaction::CreatingBox { id, origin } => {
                self.finish_box(id, origin, &pointer, &mut ch)
            }
            Interaction::CreatingArrow { id, origin, last } => {
                self.finish_arrow(id, origin, p.unwrap_or(last), &mut ch)
            }
            Interaction::Drawing { id, .. } => self.finish_drawing(id, &mut ch),
            Interaction::Translating { .. }
            | Interaction::Resizing { .. }
            | Interaction::Rotating { .. }
            | Interaction::DraggingArrowHandle { .. } => {}
            Interaction::Brushing { origin, corner } => {
                self.finish_brush(origin, p.unwrap_or(corner), &pointer, &mut ch)
            }
        }
        if self.hinted_binding_id.take().is_some() {
            ch.needs_redraw = true;
        }
        self.end_gesture(&mut ch);
        ch
    }

    /// The marquee being dragged, in page space.
    pub fn brush(&self) -> Option<Rect> {
        match &self.interaction {
            Interaction::Brushing { origin, corner } => Some(Rect::from_points(
                Point::new(origin.x, origin.y),
                Point::new(corner.x, corner.y),
            )),
            _ => None,
        }
    }

    /// The stroke being drawn; its chunks render it until release commits the shape.
    pub fn live_stroke(&self) -> Option<(AnnoShapeId, &LiveStroke)> {
        match &self.interaction {
            Interaction::Drawing { id, stroke, .. } => Some((*id, &**stroke)),
            _ => None,
        }
    }

    /// The canvas shows an arrow outside its boundary frame; this covers the inside.
    pub fn cursor_for(&self, screen: Point) -> CursorKind {
        match &self.interaction {
            Interaction::CreatingBox { .. }
            | Interaction::CreatingArrow { .. }
            | Interaction::Drawing { .. } => CursorKind::AnnotationPlus,
            Interaction::Translating { .. } => CursorKind::ClosedHand,
            Interaction::Brushing { .. } => CursorKind::Arrow,
            Interaction::Resizing { cursor, .. }
            | Interaction::DraggingArrowHandle { cursor, .. } => *cursor,
            Interaction::Rotating { center, .. } => self.rotating_cursor(*center, screen),
            Interaction::Idle => {
                let page = self.viewport.screen_to_page(screen);
                let hit = self.hit_shape(page);
                if self.editing_text_id.is_some() && hit == self.editing_text_id {
                    return CursorKind::IBeam;
                }
                match self.tool {
                    Tool::Select => self.select_hover_cursor(screen, page, hit),
                    Tool::Text if hit.is_some_and(|id| self.is_text_shape(id)) => CursorKind::IBeam,
                    _ => CursorKind::AnnotationPlus,
                }
            }
        }
    }

    fn is_text_shape(&self, id: AnnoShapeId) -> bool {
        self.doc.shape(id).is_some_and(|s| s.is_text())
    }
}
