use crate::bbox::BBox;
use crate::geometry2d::Geometry2d;
use crate::mat::Mat;
use crate::vec::PVec;
use serde::{Deserialize, Serialize};

pub const MIN_ARROW_LENGTH: f64 = 10.0;
pub const BOUND_ARROW_OFFSET: f64 = 10.0;
pub const WAY_TOO_BIG_ARROW_BEND_FACTOR: f64 = 10.0;
pub const MIN_ARROW_BEND: f64 = 8.0;
pub const NORMALIZED_ANCHOR_EPSILON: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrowhead {
    #[default]
    None,
    Arrow,
    Triangle,
    Square,
    Dot,
    Diamond,
    Inverted,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    Start,
    End,
}

/// A shape an arrow terminal is attached to. `geometry` is in the shape's local space and
/// `transform` maps that space to the page.
pub struct Bound<'a> {
    pub id: u64,
    pub geometry: &'a dyn Geometry2d,
    pub transform: Mat,
    pub page_bounds: BBox,
    pub stroke_width: f64,
    pub normalized_anchor: PVec,
    pub precise: bool,
    pub exact: bool,
}

pub struct ArrowInput<'a> {
    /// Arrow-local space to page.
    pub transform: Mat,
    pub start: PVec,
    pub end: PVec,
    pub bend: f64,
    pub head_start: Arrowhead,
    pub head_end: Arrowhead,
    pub stroke_width: f64,
    pub start_bound: Option<Bound<'a>>,
    pub end_bound: Option<Bound<'a>>,
}

pub fn is_straight(bend: f64) -> bool {
    bend.abs() < MIN_ARROW_BEND
}

/// Bends smaller than the minimum snap to a straight arrow.
pub fn snap_bend(bend: f64) -> f64 {
    if is_straight(bend) { 0.0 } else { bend }
}

/// The bend that puts the middle handle at `middle`, given the terminals.
pub fn bend_for_middle(start: PVec, end: PVec, middle: PVec) -> f64 {
    let u = PVec::sub(end, start).uni().per();
    let m = PVec::sub(middle, PVec::med(start, end));
    -(m.x * u.x + m.y * u.y)
}

/// A circular arc in SVG terms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcInfo {
    pub center: PVec,
    pub radius: f64,
    pub size: f64,
    pub length: f64,
    pub large_arc: bool,
    pub sweep: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowPoint {
    /// Where the user drags this end.
    pub handle: PVec,
    /// Where the drawn body ends, after clipping to the bound shape and the head offset.
    pub point: PVec,
    pub head: Arrowhead,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArrowBody {
    Straight { start: PVec, end: PVec },
    Arc(ArcInfo),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowInfo {
    pub start: ArrowPoint,
    pub end: ArrowPoint,
    /// The handle the user drags to bend the arrow.
    pub middle: PVec,
    pub body: ArrowBody,
    /// The arc through the handles; only set for arcs.
    pub handle_arc: Option<ArcInfo>,
    pub stroke_width: f64,
    pub is_valid: bool,
}

impl ArrowInfo {
    pub fn is_straight(&self) -> bool {
        matches!(self.body, ArrowBody::Straight { .. })
    }

    /// The terminals as they should be stored on the arrow, so a bound arrow's own start and end
    /// follow the shapes it is attached to.
    pub fn synced_terminals(&self) -> (PVec, PVec) {
        (self.start.handle, self.end.handle)
    }

    pub fn handles(&self) -> [PVec; 3] {
        [self.start.handle, self.middle, self.end.handle]
    }
}
