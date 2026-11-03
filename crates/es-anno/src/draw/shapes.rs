use super::build::BuildContext;
use super::{DrawItem, RedactionKind};
use crate::document::{Document, page_transform};
use es_geom::arrow::Terminal;
use es_geom::freehand::{self, StrokeOptions};
use es_geom::path::{DashStyle as GeoDash, DrawOptions, GeoShape, PathBuilder, geo_path};
use es_geom::{Affine, BezPath, Cap, FillRule, Join, PVec, Rect, Rgba};
use es_model::shape::{
    self, ArrowProps, DashStyle, DrawProps, FillStyle, GeoKind, GeoProps, HighlightProps,
    HighlighterProps, NumberedProps, RedactionProps, TextProps,
};
use es_model::{AnnoShape, ShapeKind};
use kurbo::{Circle, Shape};

const TEXT_BOX_RADIUS: f64 = 0.2;

pub(super) enum Built {
    Page(Vec<DrawItem>),
    Local(Vec<DrawItem>),
}

pub(super) fn build(doc: &Document, shape: &AnnoShape, cx: &BuildContext) -> Built {
    match &shape.kind {
        ShapeKind::Geo(p) => Built::Local(geo(p)),
        ShapeKind::Draw(p) => Built::Local(freehand(p)),
        ShapeKind::Arrow(p) => Built::Local(arrow(doc, shape, p)),
        ShapeKind::Text(p) => Built::Local(text(doc, p, cx)),
        ShapeKind::Numbered(p) => Built::Local(numbered(p, cx)),
        ShapeKind::Redaction(p) => Built::Page(vec![redaction(shape, p)]),
        ShapeKind::Highlight(p) => Built::Page(vec![spotlight_hole(shape, p)]),
        ShapeKind::Highlighter(p) => Built::Page(highlighter(shape, p)),
    }
}

fn stroke(path: BezPath, color: Rgba, width: f64) -> DrawItem {
    DrawItem::Stroke {
        path,
        color,
        width,
        cap: Cap::Round,
        join: Join::Round,
        dash: None,
    }
}

fn fill(path: BezPath, color: Rgba) -> DrawItem {
    DrawItem::Fill {
        path,
        color,
        rule: FillRule::NonZero,
    }
}

/// Dashes are expanded into separate subpaths here, so neither painter needs a dash sampler.
fn outline_items(
    builder: &PathBuilder,
    dash: DashStyle,
    width: f64,
    color: Rgba,
    seed: &str,
) -> Vec<DrawItem> {
    let style = match dash {
        DashStyle::Solid => return vec![stroke(builder.to_bez_path(), color, width)],
        DashStyle::Draw => {
            let path = builder.draw_path(&DrawOptions::new(width, seed));
            return vec![stroke(path, color, width)];
        }
        DashStyle::Dashed => GeoDash::Dashed,
        DashStyle::Dotted => GeoDash::Dotted,
    };
    let mut path = BezPath::new();
    for segment in builder.dashed_segments(width, style) {
        if segment.dashes.is_empty() {
            path.extend(segment.path.elements().iter().copied());
        } else {
            path.extend(kurbo::dash(
                segment.path.iter(),
                -segment.phase,
                &segment.dashes,
            ));
        }
    }
    vec![stroke(path, color, width)]
}

fn geo(p: &GeoProps) -> Vec<DrawItem> {
    let kind = match p.geo {
        GeoKind::Rectangle => GeoShape::Rectangle,
        GeoKind::Ellipse => GeoShape::Ellipse,
    };
    let builder = geo_path(kind, p.w, p.h, p.corner_radius);
    let color = p.swatch.to_rgba();
    if p.fill == FillStyle::Solid {
        return vec![fill(builder.to_bez_path(), color)];
    }
    outline_items(&builder, p.dash, p.stroke_width, color, "")
}

fn freehand(p: &DrawProps) -> Vec<DrawItem> {
    if p.points.is_empty() {
        return Vec::new();
    }
    let opts = StrokeOptions::for_draw_shape(p.is_pen, p.stroke_width, p.is_complete, false);
    let color = p.swatch.to_rgba();
    freehand::ink_chunks(&p.points, &opts)
        .into_iter()
        .map(|chunk| fill(chunk.path.as_ref().clone(), color))
        .collect()
}

fn arrow(doc: &Document, shape: &AnnoShape, p: &ArrowProps) -> Vec<DrawItem> {
    let Some(info) = doc.arrow_info(shape.id).filter(|i| i.is_valid) else {
        return Vec::new();
    };
    let color = p.swatch.to_rgba();
    let seed = shape.id.0.to_string();
    let mut items = outline_items(&info.body_builder(), p.dash, p.stroke_width, color, &seed);
    for side in [Terminal::Start, Terminal::End] {
        let Some(head) = info.head_path(side) else {
            continue;
        };
        if head.filled {
            items.push(fill(head.path.clone(), color));
        }
        items.push(stroke(head.path, color, p.stroke_width));
    }
    items
}

fn text_item(props: &TextProps, cx: &BuildContext) -> Option<DrawItem> {
    if props.text.is_empty() {
        return None;
    }
    let outline = cx.glyphs.outline(props);
    Some(DrawItem::Text {
        glyphs: outline.glyphs,
        color: props.swatch.to_rgba(),
        underline: if props.is_underline {
            outline.underline
        } else {
            Vec::new()
        },
    })
}

fn text(doc: &Document, p: &TextProps, cx: &BuildContext) -> Vec<DrawItem> {
    let mut items = Vec::new();
    if !p.text.is_empty() && (p.fill.is_some() || p.border.is_some()) {
        let size = doc.measure_text(p);
        let pad = p.padding.max(0.0);
        let rect = kurbo::RoundedRect::new(
            -pad,
            -pad,
            size.width + pad,
            size.height + pad,
            p.font_size * TEXT_BOX_RADIUS,
        )
        .to_path(0.05);
        if let Some(fill_swatch) = &p.fill {
            items.push(fill(rect.clone(), fill_swatch.to_rgba()));
        }
        if let Some(border) = &p.border {
            items.push(stroke(rect, border.swatch.to_rgba(), border.width));
        }
    }
    items.extend(text_item(p, cx));
    items
}

fn digit_scale(digits: usize) -> f64 {
    match digits {
        0..=2 => 0.54,
        3 => 0.44,
        _ => 0.34,
    }
}

fn numbered(p: &NumberedProps, cx: &BuildContext) -> Vec<DrawItem> {
    let d = p.diameter;
    let outline_width = (d * 0.055).max(1.0);
    let (ink, ring_color) = if p.swatch.prefers_dark_text() {
        (
            Rgba::new(0.0, 0.0, 0.0, 1.0),
            Rgba::new(0.0, 0.0, 0.0, 0.22),
        )
    } else {
        (
            Rgba::new(1.0, 1.0, 1.0, 1.0),
            Rgba::new(1.0, 1.0, 1.0, 0.42),
        )
    };
    let disc = Circle::new((d / 2.0, d / 2.0), d / 2.0).to_path(0.1);
    let ring = Circle::new((d / 2.0, d / 2.0), (d - outline_width) / 2.0).to_path(0.1);
    let mut items = vec![
        fill(disc, p.swatch.to_rgba()),
        stroke(ring, ring_color, outline_width),
    ];
    let value = p.value.to_string();
    let digits = TextProps {
        font_size: d * digit_scale(value.len()),
        text: value,
        swatch: es_model::Swatch {
            red: f64::from(ink.r),
            green: f64::from(ink.g),
            blue: f64::from(ink.b),
            alpha: 1.0,
            ..p.swatch.clone()
        },
        font_family: shape::FontFamily::Pro,
        is_bold: true,
        is_italic: false,
        is_underline: false,
        align: shape::TextAlign::Start,
        ..TextProps::default()
    };
    if let Some(DrawItem::Text { glyphs, color, .. }) = text_item(&digits, cx) {
        let b = glyphs.bounding_box();
        let centred = Affine::translate((d / 2.0 - b.center().x, d / 2.0 - b.center().y)) * glyphs;
        items.push(DrawItem::Text {
            glyphs: centred,
            color,
            underline: Vec::new(),
        });
    }
    items
}

fn redaction(shape: &AnnoShape, p: &RedactionProps) -> DrawItem {
    DrawItem::Redaction {
        rect: Rect::new(shape.x, shape.y, shape.x + p.w, shape.y + p.h),
        rotation: shape.rotation,
        mirrored: shape.mirrored,
        kind: match p.kind {
            shape::RedactionKind::Blur => RedactionKind::Blur,
            shape::RedactionKind::Pixelate => RedactionKind::Pixelate,
        },
        density: p.density,
    }
}

pub const HIGHLIGHTER_ALPHA: f32 = 0.4;

fn highlighter(shape: &AnnoShape, p: &HighlighterProps) -> Vec<DrawItem> {
    if p.points.len() < 2 {
        return Vec::new();
    }
    let m = page_transform(shape);
    let mut path = BezPath::new();
    for (i, pt) in p.points.iter().enumerate() {
        let q = m.apply_to_point(PVec::new(pt.x, pt.y));
        if i == 0 {
            path.move_to((q.x, q.y));
        } else {
            path.line_to((q.x, q.y));
        }
    }
    let mut color = p.swatch.to_rgba();
    color.a *= HIGHLIGHTER_ALPHA * shape.opacity as f32;
    vec![DrawItem::Highlighter {
        path,
        width: p.stroke_width,
        color,
    }]
}

fn spotlight_hole(shape: &AnnoShape, p: &HighlightProps) -> DrawItem {
    let m = page_transform(shape);
    let mut hole = BezPath::new();
    let corners = [(0.0, 0.0), (p.w, 0.0), (p.w, p.h), (0.0, p.h)];
    for (i, (x, y)) in corners.into_iter().enumerate() {
        let q = m.apply_to_point(PVec::new(x, y));
        if i == 0 {
            hole.move_to((q.x, q.y));
        } else {
            hole.line_to((q.x, q.y));
        }
    }
    hole.close_path();
    DrawItem::Spotlight { holes: vec![hole] }
}
