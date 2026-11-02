mod props;
mod tool;

pub use props::{
    ArrowProps, Arrowhead, DashStyle, DrawProps, FillStyle, FontFamily, GeoKind, GeoProps,
    HighlightProps, NumberedProps, RedactionKind, RedactionProps, TextAlign, TextProps,
};
pub use tool::Tool;

use crate::swatch::Swatch;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnnoShapeId(pub Uuid);

impl AnnoShapeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AnnoShapeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShapeKind {
    Geo(GeoProps),
    Draw(DrawProps),
    Arrow(ArrowProps),
    Text(TextProps),
    Redaction(RedactionProps),
    Highlight(HighlightProps),
    Numbered(NumberedProps),
}

impl Default for ShapeKind {
    fn default() -> Self {
        ShapeKind::Geo(GeoProps::default())
    }
}

/// Page space is the screenshot's pixel space, y down, origin top left.
/// `x`/`y` is where the shape's local origin sits; `rotation` is radians, clockwise on screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnnoShape {
    pub id: AnnoShapeId,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    /// Reflects local X; only produced by a flip.
    #[serde(skip_serializing_if = "is_false")]
    pub mirrored: bool,
    /// 0..=1 over the whole shape.
    pub opacity: f64,
    pub kind: ShapeKind,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for AnnoShape {
    fn default() -> Self {
        Self {
            id: AnnoShapeId::new(),
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            mirrored: false,
            opacity: 1.0,
            kind: ShapeKind::default(),
        }
    }
}

impl AnnoShape {
    pub fn new(x: f64, y: f64, kind: ShapeKind) -> Self {
        Self {
            x,
            y,
            kind,
            ..Self::default()
        }
    }

    pub fn tool(&self) -> Tool {
        match &self.kind {
            ShapeKind::Geo(p) => match (p.geo, p.fill) {
                (GeoKind::Rectangle, FillStyle::None) => Tool::Rectangle,
                (GeoKind::Rectangle, FillStyle::Solid) => Tool::FilledRectangle,
                (GeoKind::Ellipse, _) => Tool::Ellipse,
            },
            ShapeKind::Draw(_) => Tool::Freehand,
            ShapeKind::Arrow(p) => {
                if p.arrowhead_start == Arrowhead::None && p.arrowhead_end == Arrowhead::None {
                    Tool::Line
                } else {
                    Tool::Arrow
                }
            }
            ShapeKind::Text(_) => Tool::Text,
            ShapeKind::Redaction(p) => match p.kind {
                RedactionKind::Blur => Tool::Blur,
                RedactionKind::Pixelate => Tool::Pixelate,
            },
            ShapeKind::Highlight(_) => Tool::Highlight,
            ShapeKind::Numbered(_) => Tool::NumberedCircle,
        }
    }

    pub fn swatch(&self) -> Option<&Swatch> {
        match &self.kind {
            ShapeKind::Geo(p) => Some(&p.swatch),
            ShapeKind::Draw(p) => Some(&p.swatch),
            ShapeKind::Arrow(p) => Some(&p.swatch),
            ShapeKind::Text(p) => Some(&p.swatch),
            ShapeKind::Numbered(p) => Some(&p.swatch),
            ShapeKind::Redaction(_) | ShapeKind::Highlight(_) => None,
        }
    }

    pub fn stroke_width(&self) -> f64 {
        match &self.kind {
            ShapeKind::Geo(p) => p.stroke_width,
            ShapeKind::Draw(p) => p.stroke_width,
            ShapeKind::Arrow(p) => p.stroke_width,
            _ => 0.0,
        }
    }

    /// Whether a click anywhere inside grabs the shape, as opposed to only its outline.
    pub fn is_filled(&self) -> bool {
        match &self.kind {
            ShapeKind::Geo(p) => p.fill != FillStyle::None,
            ShapeKind::Draw(p) => p.is_closed,
            ShapeKind::Arrow(_) => false,
            ShapeKind::Text(_)
            | ShapeKind::Redaction(_)
            | ShapeKind::Highlight(_)
            | ShapeKind::Numbered(_) => true,
        }
    }

    pub fn is_arrow(&self) -> bool {
        matches!(self.kind, ShapeKind::Arrow(_))
    }

    pub fn is_text(&self) -> bool {
        matches!(self.kind, ShapeKind::Text(_))
    }

    pub fn is_redaction(&self) -> bool {
        matches!(self.kind, ShapeKind::Redaction(_))
    }

    pub fn is_highlight(&self) -> bool {
        matches!(self.kind, ShapeKind::Highlight(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowTerminal {
    Start,
    End,
}

/// Where on the bound shape's bounds the terminal points, 0..=1 of its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedAnchor {
    pub x: f64,
    pub y: f64,
}

impl Default for NormalizedAnchor {
    fn default() -> Self {
        Self { x: 0.5, y: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArrowBinding {
    pub arrow_id: AnnoShapeId,
    pub to_id: AnnoShapeId,
    pub terminal: ArrowTerminal,
    pub normalized_anchor: NormalizedAnchor,
    /// When false the terminal snaps to the shape's centre instead of the anchor.
    pub is_precise: bool,
    /// When true the arrow stops exactly at the terminal instead of at the shape's edge.
    pub is_exact: bool,
}

impl Default for ArrowBinding {
    fn default() -> Self {
        Self {
            arrow_id: AnnoShapeId::new(),
            to_id: AnnoShapeId::new(),
            terminal: ArrowTerminal::Start,
            normalized_anchor: NormalizedAnchor::default(),
            is_precise: false,
            is_exact: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_geom::PVec;
    use serde_json::json;

    fn shape(kind: ShapeKind) -> AnnoShape {
        AnnoShape::new(3.0, 4.0, kind)
    }

    #[test]
    fn every_kind_maps_to_the_tool_that_draws_it() {
        let geo = |geo, fill| {
            shape(ShapeKind::Geo(GeoProps {
                geo,
                fill,
                ..GeoProps::default()
            }))
        };
        assert_eq!(
            geo(GeoKind::Rectangle, FillStyle::None).tool(),
            Tool::Rectangle
        );
        assert_eq!(
            geo(GeoKind::Rectangle, FillStyle::Solid).tool(),
            Tool::FilledRectangle
        );
        assert_eq!(
            geo(GeoKind::Ellipse, FillStyle::Solid).tool(),
            Tool::Ellipse
        );
        assert_eq!(
            shape(ShapeKind::Draw(DrawProps::default())).tool(),
            Tool::Freehand
        );
        assert_eq!(
            shape(ShapeKind::Arrow(ArrowProps::default())).tool(),
            Tool::Arrow
        );
        let line = ArrowProps {
            arrowhead_end: Arrowhead::None,
            ..ArrowProps::default()
        };
        assert_eq!(shape(ShapeKind::Arrow(line)).tool(), Tool::Line);
        let both_ends = ArrowProps {
            arrowhead_start: Arrowhead::Dot,
            arrowhead_end: Arrowhead::None,
            ..ArrowProps::default()
        };
        assert_eq!(shape(ShapeKind::Arrow(both_ends)).tool(), Tool::Arrow);
        assert_eq!(
            shape(ShapeKind::Text(TextProps::default())).tool(),
            Tool::Text
        );
        let pix = RedactionProps {
            kind: RedactionKind::Pixelate,
            ..RedactionProps::default()
        };
        assert_eq!(shape(ShapeKind::Redaction(pix)).tool(), Tool::Pixelate);
        assert_eq!(
            shape(ShapeKind::Redaction(RedactionProps::default())).tool(),
            Tool::Blur
        );
        assert_eq!(
            shape(ShapeKind::Highlight(HighlightProps::default())).tool(),
            Tool::Highlight
        );
        assert_eq!(
            shape(ShapeKind::Numbered(NumberedProps::default())).tool(),
            Tool::NumberedCircle
        );
    }

    #[test]
    fn hollow_shapes_are_grabbed_by_outline_only() {
        assert!(!shape(ShapeKind::Geo(GeoProps::default())).is_filled());
        assert!(!shape(ShapeKind::Arrow(ArrowProps::default())).is_filled());
        assert!(!shape(ShapeKind::Draw(DrawProps::default())).is_filled());
        let closed = DrawProps {
            is_closed: true,
            ..DrawProps::default()
        };
        assert!(shape(ShapeKind::Draw(closed)).is_filled());
        assert!(shape(ShapeKind::Text(TextProps::default())).is_filled());
        assert!(shape(ShapeKind::Numbered(NumberedProps::default())).is_filled());
    }

    #[test]
    fn prop_defaults_equal_the_documented_ones() {
        let g = GeoProps::default();
        assert_eq!(
            (g.w, g.h, g.stroke_width, g.corner_radius),
            (100.0, 100.0, 8.0, 0.0)
        );
        assert_eq!(
            (g.fill, g.dash, g.geo),
            (FillStyle::None, DashStyle::Solid, GeoKind::Rectangle)
        );
        let a = ArrowProps::default();
        assert_eq!(
            (a.start, a.end),
            (PVec::new(0.0, 0.0), PVec::new(100.0, 0.0))
        );
        assert_eq!(
            (a.arrowhead_start, a.arrowhead_end),
            (Arrowhead::None, Arrowhead::Arrow)
        );
        let t = TextProps::default();
        assert_eq!((t.font_size, t.w), (48.0, 16.0));
        assert!(t.is_bold && t.auto_size && !t.is_italic && !t.is_underline);
        assert_eq!(
            (t.font_family, t.align),
            (FontFamily::Pro, TextAlign::Start)
        );
        let r = RedactionProps::default();
        assert_eq!((r.kind, r.density), (RedactionKind::Blur, 0.55));
        assert_eq!(NumberedProps::default().diameter, 64.0);
        let s = AnnoShape::default();
        assert_eq!((s.rotation, s.opacity, s.mirrored), (0.0, 1.0, false));
    }

    #[test]
    fn kind_is_internally_tagged_in_snake_case() {
        let s = shape(ShapeKind::Numbered(NumberedProps::default()));
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"]["type"], "numbered");
        assert_eq!(v["kind"]["value"], 1);
        assert!(v.get("mirrored").is_none());
        assert_eq!(serde_json::from_value::<AnnoShape>(v).unwrap(), s);
    }

    #[test]
    fn missing_fields_take_defaults_and_unknown_fields_are_ignored() {
        let id = Uuid::new_v4();
        let v = json!({
            "id": id,
            "x": 10.0,
            "y": 20.0,
            "future": [1, 2],
            "kind": {"type": "geo", "w": 40.0, "from_the_future": true}
        });
        let s: AnnoShape = serde_json::from_value(v).unwrap();
        assert_eq!(s.id, AnnoShapeId(id));
        assert_eq!(s.opacity, 1.0);
        match s.kind {
            ShapeKind::Geo(g) => assert_eq!((g.w, g.h, g.stroke_width), (40.0, 100.0, 8.0)),
            other => panic!("unexpected kind {other:?}"),
        }
    }

    #[test]
    fn mirrored_round_trips_when_set() {
        let mut s = shape(ShapeKind::Highlight(HighlightProps::default()));
        s.mirrored = true;
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["mirrored"], true);
        assert!(serde_json::from_value::<AnnoShape>(v).unwrap().mirrored);
    }
}
