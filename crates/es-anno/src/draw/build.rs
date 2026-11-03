use super::{DrawItem, DrawList, shapes};
use crate::document::{Document, page_transform};
use es_geom::{Affine, BezPath, Rect};
use es_model::{AnnoShape, AnnoShapeId, TextProps};
use i_overlay::core::fill_rule::FillRule as OverlayFillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::PathEl;

/// Black at this alpha dims everything outside the spotlight holes.
pub const SPOTLIGHT_DIM: f32 = 0.55;

#[derive(Debug, Clone, Default)]
pub struct GlyphOutline {
    pub glyphs: BezPath,
    pub underline: Vec<Rect>,
}

pub trait GlyphOutliner {
    /// Glyph outlines laid out from the text box's top-left corner, in local units.
    fn outline(&self, props: &TextProps) -> GlyphOutline;
}

pub struct BuildContext<'a> {
    pub glyphs: &'a dyn GlyphOutliner,
    /// Shapes left out of the list, such as a text shape being edited in place.
    pub skip: &'a [AnnoShapeId],
}

impl<'a> BuildContext<'a> {
    pub fn new(glyphs: &'a dyn GlyphOutliner) -> Self {
        Self { glyphs, skip: &[] }
    }
}

/// Redactions and spotlight holes come back in page space; everything else is one `Group` in local
/// space carrying the shape's transform and opacity. Cacheable by `Document::revision`.
pub fn draw_items(doc: &Document, shape: &AnnoShape, cx: &BuildContext) -> Vec<DrawItem> {
    match shapes::build(doc, shape, cx) {
        shapes::Built::Page(items) => items,
        shapes::Built::Local(items) if items.is_empty() => items,
        shapes::Built::Local(items) => {
            let m = page_transform(shape);
            vec![DrawItem::Group {
                transform: Affine::new([m.a, m.b, m.c, m.d, m.e, m.f]),
                opacity: shape.opacity,
                items,
            }]
        }
    }
}

/// Highlighter strokes, redactions, then one spotlight over all highlight holes, then the rest in z-order.
pub fn build_draw_list(doc: &Document, cx: &BuildContext) -> DrawList {
    let shapes: Vec<&AnnoShape> = doc
        .shapes()
        .iter()
        .filter(|s| !cx.skip.contains(&s.id))
        .collect();
    let mut items = Vec::new();
    for s in shapes.iter().filter(|s| s.is_highlighter()) {
        items.extend(draw_items(doc, s, cx));
    }
    for s in shapes.iter().filter(|s| s.is_redaction()) {
        items.extend(draw_items(doc, s, cx));
    }
    let holes: Vec<BezPath> = shapes
        .iter()
        .filter(|s| s.is_highlight())
        .flat_map(|s| draw_items(doc, s, cx))
        .flat_map(|item| match item {
            DrawItem::Spotlight { holes } => holes,
            _ => Vec::new(),
        })
        .collect();
    let holes = union_holes(&holes);
    if !holes.is_empty() {
        items.push(DrawItem::Spotlight { holes });
    }
    for s in shapes
        .iter()
        .filter(|s| !s.is_redaction() && !s.is_highlight() && !s.is_highlighter())
    {
        items.extend(draw_items(doc, s, cx));
    }
    DrawList { items }
}

fn polygon(path: &BezPath) -> Vec<[f64; 2]> {
    path.elements()
        .iter()
        .filter_map(|el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => Some([p.x, p.y]),
            _ => None,
        })
        .collect()
}

fn signed_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        / 2.0
}

/// One path per connected region; an enclosed gap is a further subpath of the same path.
fn union_holes(holes: &[BezPath]) -> Vec<BezPath> {
    let contours: Vec<Vec<[f64; 2]>> = holes
        .iter()
        .map(polygon)
        .filter(|p| p.len() >= 3)
        .map(|mut p| {
            if signed_area(&p) < 0.0 {
                p.reverse();
            }
            p
        })
        .collect();
    if contours.is_empty() {
        return Vec::new();
    }
    let clip: Vec<[f64; 2]> = Vec::new();
    contours
        .overlay(&clip, OverlayRule::Subject, OverlayFillRule::NonZero)
        .into_iter()
        .map(|shape| {
            let mut path = BezPath::new();
            for contour in shape {
                let mut points = contour.into_iter();
                if let Some(first) = points.next() {
                    path.move_to((first[0], first[1]));
                    for p in points {
                        path.line_to((p[0], p[1]));
                    }
                    path.close_path();
                }
            }
            path
        })
        .collect()
}
