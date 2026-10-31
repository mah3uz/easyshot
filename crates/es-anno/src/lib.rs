pub mod crop;
pub mod document;
pub mod draw;
pub mod editor;
pub mod style;

use es_geom::Point;
use smallvec::SmallVec;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unsupported on this system")]
    Unsupported,
    #[error("cancelled")]
    Cancelled,
    #[error("timed out")]
    Timeout,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("i/o error on {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse { line: usize, col: usize, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerButton {
    #[default]
    Primary,
    Secondary,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerInfo {
    pub screen: Point,
    pub page: Point,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub is_pen: bool,
    pub pressure: f64,
    pub click_count: u8,
    pub button: PointerButton,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub dirty_shapes: SmallVec<[ShapeId; 4]>,
    pub all_dirty: bool,
    pub selection_changed: bool,
    pub tool_changed: bool,
    pub undo_state_changed: bool,
    pub needs_redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorKind {
    Arrow,
    AnnotationPlus,
    OpenHand,
    ClosedHand,
    IBeam,
    Crosshair,
    Resize { screen_angle_deg: f32 },
    Rotate { octant: u8 },
}
