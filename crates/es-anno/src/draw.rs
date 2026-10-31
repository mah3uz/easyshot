use es_geom::{Affine, BezPath, Cap, FillRule, Join, Rect, Rgba};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactionKind {
    Blur,
    Pixelate,
}

#[derive(Debug, Clone)]
pub enum DrawItem {
    Fill {
        path: BezPath,
        color: Rgba,
        rule: FillRule,
    },
    Stroke {
        path: BezPath,
        color: Rgba,
        width: f64,
        cap: Cap,
        join: Join,
        dash: Option<(Vec<f64>, f64)>,
    },
    Redaction {
        rect: Rect,
        rotation: f64,
        mirrored: bool,
        kind: RedactionKind,
        density: f64,
    },
    Spotlight {
        holes: Vec<BezPath>,
    },
    Text {
        glyphs: BezPath,
        color: Rgba,
        underline: Vec<Rect>,
    },
    /// `opacity` is a per-item alpha multiply in both painters, not a transparency layer.
    Group {
        transform: Affine,
        opacity: f64,
        items: Vec<DrawItem>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub items: Vec<DrawItem>,
}
