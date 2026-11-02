#![allow(dead_code)]

use es_anno::{Changes, Editor, GestureId, ShapeId, TextMeasure, Viewport};
use es_geom::{PVec, Point, Size};
use es_model::{
    AnnoShape, AnnoShapeId, AnnotationDocument, ArrowBinding, ArrowProps, ArrowTerminal, FillStyle,
    GeoProps, HighlightProps, RedactionProps, ShapeKind, StylePreset, TextProps, Tool,
};

pub struct FakeMeasure;

impl TextMeasure for FakeMeasure {
    fn measure(&self, p: &TextProps) -> Size {
        let lines: Vec<&str> = p.text.split('\n').collect();
        let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let w = (widest as f64 * p.font_size * 0.5).ceil() + 1.0;
        Size::new(
            w.max(16.0),
            p.font_size.max(1.0) * 1.35 * lines.len() as f64,
        )
    }
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Geo(GeoProps {
            w,
            h,
            ..GeoProps::default()
        }),
    )
}

pub fn filled_rect(x: f64, y: f64, w: f64, h: f64) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Geo(GeoProps {
            w,
            h,
            fill: FillStyle::Solid,
            ..GeoProps::default()
        }),
    )
}

pub fn redaction(x: f64, y: f64) -> AnnoShape {
    AnnoShape::new(x, y, ShapeKind::Redaction(RedactionProps::default()))
}

pub fn spotlight(x: f64, y: f64) -> AnnoShape {
    AnnoShape::new(x, y, ShapeKind::Highlight(HighlightProps::default()))
}

pub fn arrow(x: f64, y: f64, start: (f64, f64), end: (f64, f64)) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Arrow(ArrowProps {
            start: PVec::new(start.0, start.1),
            end: PVec::new(end.0, end.1),
            ..ArrowProps::default()
        }),
    )
}

pub fn text(x: f64, y: f64, s: &str) -> AnnoShape {
    AnnoShape::new(
        x,
        y,
        ShapeKind::Text(TextProps {
            text: s.into(),
            ..TextProps::default()
        }),
    )
}

pub fn bind(arrow: AnnoShapeId, to: AnnoShapeId, terminal: ArrowTerminal) -> ArrowBinding {
    ArrowBinding {
        arrow_id: arrow,
        to_id: to,
        terminal,
        ..ArrowBinding::default()
    }
}

pub fn model(shapes: Vec<AnnoShape>, bindings: Vec<ArrowBinding>) -> AnnotationDocument {
    AnnotationDocument {
        shapes,
        bindings,
        ..AnnotationDocument::default()
    }
}

/// The select tool, on an 1800 x 1000 image shown at 1:1.
pub fn editor(shapes: Vec<AnnoShape>, bindings: Vec<ArrowBinding>) -> Editor {
    let preset = StylePreset {
        tool: Tool::Select,
        ..StylePreset::default()
    };
    let mut ed = Editor::new(model(shapes, bindings), Box::new(FakeMeasure), &preset);
    ed.set_viewport(Viewport {
        origin: Point::ZERO,
        scale: 1.0,
        image_size: Size::new(1800.0, 1000.0),
    });
    ed
}

pub fn select(ed: &mut Editor, ids: &[AnnoShapeId]) {
    ed.select(ids.iter().copied());
}

pub fn order(ed: &Editor) -> Vec<AnnoShapeId> {
    ed.doc().shapes().iter().map(|s| s.id).collect()
}

pub fn handles(ed: &Editor, ids: &[AnnoShapeId]) -> Vec<ShapeId> {
    ids.iter()
        .map(|id| ed.doc().handle(*id).expect("shape in document"))
        .collect()
}

pub fn g(n: u64) -> GestureId {
    GestureId(n)
}

pub fn changes(f: impl FnOnce(&mut Changes)) -> Changes {
    let mut c = Changes::default();
    f(&mut c);
    c
}

use es_anno::{PointerButton, PointerInfo, Snapshot};

/// A mouse press at a page point; the common editor shows the page at 1:1 from the origin.
pub fn at(x: f64, y: f64) -> PointerInfo {
    PointerInfo {
        screen: Point::new(x, y),
        page: Point::new(x, y),
        shift: false,
        alt: false,
        ctrl: false,
        is_pen: false,
        pressure: 0.0,
        click_count: 1,
        button: PointerButton::Primary,
    }
}

/// Down at `from`, through the midpoint, up at `to`; the moves and the release carry `to`'s keys.
pub fn drag(ed: &mut Editor, from: PointerInfo, to: PointerInfo) {
    ed.pointer_down(from);
    let mid = Point::new(
        (from.page.x + to.page.x) / 2.0,
        (from.page.y + to.page.y) / 2.0,
    );
    ed.pointer_move(PointerInfo {
        screen: mid,
        page: mid,
        ..to
    });
    ed.pointer_move(to);
    ed.pointer_up(to);
}

/// The gesture made exactly one undo entry: one undo returns to `before`, redo returns to now.
pub fn one_undo_entry(ed: &mut Editor, before: &Snapshot) {
    let after = ed.doc().snapshot();
    assert_ne!(&after, before, "the gesture must change the document");
    ed.undo();
    assert_eq!(&ed.doc().snapshot(), before);
    ed.redo();
    assert_eq!(ed.doc().snapshot(), after);
}
