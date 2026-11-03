use crate::common;

use common::{
    FakeMeasure, FakeOutliner, arrow, filled_rect, model, rect, redaction, spotlight, text,
};
use es_anno::draw::{DrawItem, RedactionKind, draw_items};
use es_anno::{BuildContext, Document, build_draw_list};
use es_geom::{BezPath, Cap, FillRule, Join, PVec};
use es_model::shape::{DashStyle, GeoKind, NumberedProps, TextProps};
use es_model::{AnnoShape, ArrowProps, Arrowhead, DrawProps, ShapeKind};
use kurbo::{PathEl, Shape};

fn doc(shapes: Vec<AnnoShape>) -> Document {
    Document::new(model(shapes, vec![]), Box::new(FakeMeasure))
}

fn list(doc: &Document) -> Vec<DrawItem> {
    build_draw_list(doc, &BuildContext::new(&FakeOutliner)).items
}

fn items_of(shape: AnnoShape) -> Vec<DrawItem> {
    let d = doc(vec![shape]);
    let shape = d.shapes()[0].clone();
    draw_items(&d, &shape, &BuildContext::new(&FakeOutliner))
}

fn inner(items: Vec<DrawItem>) -> Vec<DrawItem> {
    match items.into_iter().next() {
        Some(DrawItem::Group { items, .. }) => items,
        other => panic!("expected a group, got {other:?}"),
    }
}

fn moves(path: &BezPath) -> usize {
    path.elements()
        .iter()
        .filter(|e| matches!(e, PathEl::MoveTo(_)))
        .count()
}

fn kind(item: &DrawItem) -> &'static str {
    match item {
        DrawItem::Fill { .. } => "fill",
        DrawItem::Stroke { .. } => "stroke",
        DrawItem::Redaction { .. } => "redaction",
        DrawItem::Spotlight { .. } => "spotlight",
        DrawItem::Text { .. } => "text",
        DrawItem::Group { .. } => "group",
    }
}

fn kinds(items: &[DrawItem]) -> Vec<&'static str> {
    items.iter().map(kind).collect()
}

fn dashed(style: DashStyle) -> AnnoShape {
    let mut s = rect(10.0, 10.0, 200.0, 100.0);
    if let ShapeKind::Geo(p) = &mut s.kind {
        p.dash = style;
    }
    s
}

#[test]
fn a_stroked_rectangle_is_one_round_stroke_in_a_group_carrying_the_shape_transform() {
    let mut s = rect(30.0, 40.0, 200.0, 100.0);
    s.opacity = 0.5;
    s.rotation = 0.25;
    let items = items_of(s);
    assert_eq!(kinds(&items), ["group"]);
    let DrawItem::Group {
        transform,
        opacity,
        items,
    } = &items[0]
    else {
        unreachable!()
    };
    assert_eq!(*opacity, 0.5);
    let c = transform.as_coeffs();
    assert_eq!((c[4], c[5]), (30.0, 40.0));
    assert!((c[0] - 0.25f64.cos()).abs() < 1e-12);
    let [
        DrawItem::Stroke {
            path,
            width,
            cap,
            join,
            dash,
            ..
        },
    ] = items.as_slice()
    else {
        panic!("{items:?}");
    };
    assert_eq!((*width, *cap, *join), (8.0, Cap::Round, Join::Round));
    assert!(dash.is_none());
    assert_eq!(moves(path), 1);
    assert_eq!(path.bounding_box().size(), kurbo::Size::new(200.0, 100.0));
}

#[test]
fn a_filled_rectangle_is_one_non_zero_fill_and_no_outline() {
    let items = inner(items_of(filled_rect(0.0, 0.0, 120.0, 60.0)));
    let [DrawItem::Fill { path, rule, color }] = items.as_slice() else {
        panic!("{items:?}");
    };
    assert_eq!(*rule, FillRule::NonZero);
    assert_eq!(color.a, 1.0);
    assert_eq!(path.bounding_box().size(), kurbo::Size::new(120.0, 60.0));
}

#[test]
fn an_ellipse_fills_its_box_with_curves() {
    let mut s = filled_rect(0.0, 0.0, 150.0, 80.0);
    if let ShapeKind::Geo(p) = &mut s.kind {
        p.geo = GeoKind::Ellipse;
    }
    let items = inner(items_of(s));
    let [DrawItem::Fill { path, .. }] = items.as_slice() else {
        panic!("{items:?}");
    };
    let b = path.bounding_box();
    assert!((b.width() - 150.0).abs() < 0.5 && (b.height() - 80.0).abs() < 0.5);
    assert!(
        path.elements()
            .iter()
            .any(|e| matches!(e, PathEl::CurveTo(..)))
    );
}

#[test]
fn a_dashed_outline_is_expanded_into_many_subpaths_with_no_dash_array_left_for_the_painter() {
    let items = inner(items_of(dashed(DashStyle::Dashed)));
    let [DrawItem::Stroke { path, dash, .. }] = items.as_slice() else {
        panic!("{items:?}");
    };
    assert!(dash.is_none());
    assert!(moves(path) > 4, "{} subpaths", moves(path));
    let solid = inner(items_of(dashed(DashStyle::Solid)));
    let DrawItem::Stroke { path: whole, .. } = &solid[0] else {
        panic!()
    };
    assert_eq!(moves(whole), 1);
}

#[test]
fn dotted_makes_more_pieces_than_dashed() {
    let count = |style| match &inner(items_of(dashed(style)))[0] {
        DrawItem::Stroke { path, .. } => moves(path),
        other => panic!("{other:?}"),
    };
    assert!(count(DashStyle::Dotted) > count(DashStyle::Dashed));
}

#[test]
fn a_dashed_outline_has_a_half_dash_at_the_corner_so_the_pattern_is_centred_there() {
    let items = inner(items_of(dashed(DashStyle::Dashed)));
    let DrawItem::Stroke { path, .. } = &items[0] else {
        panic!()
    };
    let els = path.elements();
    let corner = els.windows(2).find_map(|w| match (w[0], w[1]) {
        (PathEl::MoveTo(a), PathEl::LineTo(b)) if a.x == 0.0 && a.y == 0.0 => Some(b),
        _ => None,
    });
    let end = corner.expect("a dash starting at the corner");
    assert!(end.y == 0.0 && end.x > 4.0 && end.x < 16.0, "{end:?}");
}

#[test]
fn the_hand_drawn_style_strokes_a_jittered_path() {
    let items = inner(items_of(dashed(DashStyle::Draw)));
    let [DrawItem::Stroke { path, dash, .. }] = items.as_slice() else {
        panic!("{items:?}");
    };
    assert!(dash.is_none());
    assert!(path.elements().len() > 5);
}

#[test]
fn an_arrow_is_a_body_stroke_then_a_stroke_for_an_open_head() {
    let items = inner(items_of(arrow(0.0, 0.0, (0.0, 0.0), (200.0, 0.0))));
    assert_eq!(kinds(&items), ["stroke", "stroke"]);
}

fn arrow_with(start: Arrowhead, end: Arrowhead) -> AnnoShape {
    AnnoShape::new(
        0.0,
        0.0,
        ShapeKind::Arrow(ArrowProps {
            end: PVec::new(200.0, 0.0),
            arrowhead_start: start,
            arrowhead_end: end,
            ..ArrowProps::default()
        }),
    )
}

#[test]
fn filled_heads_add_a_fill_before_their_outline_and_each_end_gets_its_own() {
    let items = inner(items_of(arrow_with(Arrowhead::Dot, Arrowhead::Triangle)));
    assert_eq!(
        kinds(&items),
        ["stroke", "fill", "stroke", "fill", "stroke"]
    );
}

#[test]
fn a_line_without_heads_is_only_a_body() {
    let items = inner(items_of(arrow_with(Arrowhead::None, Arrowhead::None)));
    assert_eq!(kinds(&items), ["stroke"]);
}

#[test]
fn a_dashed_arrow_body_is_expanded_but_its_head_stays_whole() {
    let mut s = arrow_with(Arrowhead::None, Arrowhead::Triangle);
    if let ShapeKind::Arrow(p) = &mut s.kind {
        p.dash = DashStyle::Dashed;
        p.end = PVec::new(400.0, 0.0);
    }
    let items = inner(items_of(s));
    let DrawItem::Stroke { path, .. } = &items[0] else {
        panic!()
    };
    assert!(moves(path) > 3);
    let DrawItem::Stroke { path: head, .. } = &items[2] else {
        panic!("{items:?}");
    };
    assert_eq!(moves(head), 1);
}

#[test]
fn a_long_freehand_stroke_becomes_several_non_zero_fills_and_a_single_point_still_draws() {
    let points: Vec<PVec> = (0..400)
        .map(|i| {
            let t = f64::from(i);
            PVec::new(t * 2.0, (t / 9.0).sin() * 40.0)
        })
        .collect();
    let s = AnnoShape::new(
        10.0,
        10.0,
        ShapeKind::Draw(DrawProps {
            points,
            is_complete: true,
            ..DrawProps::default()
        }),
    );
    let items = inner(items_of(s));
    assert!(items.len() > 1, "{} chunks", items.len());
    for item in &items {
        let DrawItem::Fill { rule, path, .. } = item else {
            panic!("{item:?}");
        };
        assert_eq!(*rule, FillRule::NonZero);
        assert!(path.elements().len() > 4);
    }
    let dot = AnnoShape::new(
        0.0,
        0.0,
        ShapeKind::Draw(DrawProps {
            points: vec![PVec::new(0.0, 0.0)],
            is_complete: true,
            ..DrawProps::default()
        }),
    );
    assert_eq!(kinds(&inner(items_of(dot))), ["fill"]);
    let empty = AnnoShape::new(0.0, 0.0, ShapeKind::Draw(DrawProps::default()));
    assert!(items_of(empty).is_empty());
}

#[test]
fn a_numbered_badge_is_a_disc_a_ring_and_digits_centred_on_the_disc() {
    let s = AnnoShape::new(
        0.0,
        0.0,
        ShapeKind::Numbered(NumberedProps {
            value: 12,
            diameter: 64.0,
            ..NumberedProps::default()
        }),
    );
    let items = inner(items_of(s));
    assert_eq!(kinds(&items), ["fill", "stroke", "text"]);
    let DrawItem::Text { glyphs, .. } = &items[2] else {
        panic!()
    };
    let b = glyphs.bounding_box();
    assert!((b.center().x - 32.0).abs() < 1e-9 && (b.center().y - 32.0).abs() < 1e-9);
    assert!((b.height() - 64.0 * 0.54).abs() < 1e-9);
}

#[test]
fn badge_ink_and_ring_contrast_with_the_disc() {
    let colours = |swatch: es_model::Swatch| {
        let s = AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Numbered(NumberedProps {
                swatch,
                ..NumberedProps::default()
            }),
        );
        let items = inner(items_of(s));
        let (DrawItem::Stroke { color: ring, .. }, DrawItem::Text { color: ink, .. }) =
            (&items[1], &items[2])
        else {
            panic!("{items:?}")
        };
        (*ring, *ink)
    };
    let palette = es_model::Swatch::palette();
    let light = palette
        .iter()
        .find(|s| s.prefers_dark_text())
        .cloned()
        .unwrap();
    let dark = palette
        .iter()
        .find(|s| !s.prefers_dark_text())
        .cloned()
        .unwrap();
    let (ring, ink) = colours(light);
    assert_eq!((ink.r, ink.g, ink.b, ink.a), (0.0, 0.0, 0.0, 1.0));
    assert_eq!((ring.r, ring.g, ring.b, ring.a), (0.0, 0.0, 0.0, 0.22));
    let (ring, ink) = colours(dark);
    assert_eq!((ink.r, ink.g, ink.b, ink.a), (1.0, 1.0, 1.0, 1.0));
    assert_eq!((ring.r, ring.g, ring.b, ring.a), (1.0, 1.0, 1.0, 0.42));
}

#[test]
fn badge_digits_shrink_as_the_number_grows() {
    let height = |value| {
        let s = AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Numbered(NumberedProps {
                value,
                ..NumberedProps::default()
            }),
        );
        match &inner(items_of(s))[2] {
            DrawItem::Text { glyphs, .. } => glyphs.bounding_box().height(),
            other => panic!("{other:?}"),
        }
    };
    assert!(height(5) > height(123) && height(123) > height(1234));
}

#[test]
fn text_draws_glyph_outlines_in_its_colour_and_underlines_only_when_asked() {
    let items = inner(items_of(text(5.0, 6.0, "abc")));
    let [
        DrawItem::Text {
            glyphs,
            color,
            underline,
        },
    ] = items.as_slice()
    else {
        panic!("{items:?}");
    };
    assert_eq!(
        glyphs.bounding_box().width(),
        3.0 * es_model::shape::DEFAULT_TEXT_SIZE / 2.0
    );
    assert_eq!(*color, es_model::Swatch::default().to_rgba());
    assert!(underline.is_empty());

    let mut s = text(0.0, 0.0, "abc");
    if let ShapeKind::Text(p) = &mut s.kind {
        p.is_underline = true;
    }
    let items = inner(items_of(s));
    let DrawItem::Text { underline, .. } = &items[0] else {
        panic!()
    };
    assert_eq!(underline.len(), 1);
}

#[test]
fn empty_text_draws_nothing() {
    let s = AnnoShape::new(0.0, 0.0, ShapeKind::Text(TextProps::default()));
    assert!(items_of(s).is_empty());
}

#[test]
fn a_redaction_is_a_page_space_item_with_its_rect_rotation_and_density() {
    let mut s = redaction(40.0, 50.0);
    s.rotation = 0.5;
    s.mirrored = true;
    let items = items_of(s);
    let [
        DrawItem::Redaction {
            rect,
            rotation,
            mirrored,
            kind,
            density,
        },
    ] = items.as_slice()
    else {
        panic!("{items:?}");
    };
    assert_eq!(
        (rect.x0, rect.y0, rect.x1, rect.y1),
        (40.0, 50.0, 140.0, 150.0)
    );
    assert_eq!(
        (*rotation, *mirrored, *kind, *density),
        (0.5, true, RedactionKind::Blur, 0.55)
    );
}

#[test]
fn a_rotated_highlight_punches_a_rotated_hole() {
    let mut s = spotlight(100.0, 100.0);
    s.rotation = std::f64::consts::FRAC_PI_2;
    let items = items_of(s);
    let [DrawItem::Spotlight { holes }] = items.as_slice() else {
        panic!("{items:?}");
    };
    let b = holes[0].bounding_box();
    assert!((b.x0 - 0.0).abs() < 1e-9 && (b.x1 - 100.0).abs() < 1e-9);
    assert!((b.y0 - 100.0).abs() < 1e-9 && (b.y1 - 200.0).abs() < 1e-9);
}

fn spot(x: f64, y: f64, w: f64, h: f64) -> AnnoShape {
    let mut s = spotlight(x, y);
    if let ShapeKind::Highlight(p) = &mut s.kind {
        p.w = w;
        p.h = h;
    }
    s
}

#[test]
fn overlapping_highlights_are_one_spotlight_with_one_unioned_hole() {
    let d = doc(vec![
        spot(0.0, 0.0, 100.0, 100.0),
        spot(50.0, 50.0, 100.0, 100.0),
    ]);
    let items = list(&d);
    let [DrawItem::Spotlight { holes }] = items.as_slice() else {
        panic!("{:?}", kinds(&items));
    };
    assert_eq!(holes.len(), 1);
    let b = holes[0].bounding_box();
    assert_eq!((b.x0, b.y0, b.x1, b.y1), (0.0, 0.0, 150.0, 150.0));
    assert_eq!(moves(&holes[0]), 1);
    assert!(holes[0].elements().len() >= 8);
}

#[test]
fn highlights_that_are_mirrored_still_union_instead_of_cancelling() {
    let mut a = spot(0.0, 0.0, 100.0, 100.0);
    a.mirrored = true;
    let d = doc(vec![a, spot(-50.0, 20.0, 100.0, 60.0)]);
    let items = list(&d);
    let [DrawItem::Spotlight { holes }] = items.as_slice() else {
        panic!()
    };
    assert_eq!(holes.len(), 1);
}

#[test]
fn separate_highlights_stay_separate_holes_in_the_same_spotlight() {
    let d = doc(vec![
        spot(0.0, 0.0, 50.0, 50.0),
        spot(300.0, 0.0, 50.0, 50.0),
    ]);
    let items = list(&d);
    let [DrawItem::Spotlight { holes }] = items.as_slice() else {
        panic!()
    };
    assert_eq!(holes.len(), 2);
}

#[test]
fn draw_order_is_redactions_then_the_spotlight_then_the_rest_in_z_order() {
    let d = doc(vec![
        rect(0.0, 0.0, 10.0, 10.0),
        spotlight(0.0, 0.0),
        redaction(0.0, 0.0),
        filled_rect(5.0, 5.0, 10.0, 10.0),
        redaction(9.0, 9.0),
        spotlight(80.0, 80.0),
    ]);
    let items = list(&d);
    assert_eq!(
        kinds(&items),
        ["redaction", "redaction", "spotlight", "group", "group"]
    );
    let DrawItem::Group { items: first, .. } = &items[3] else {
        panic!()
    };
    assert_eq!(kinds(first), ["stroke"]);
    let DrawItem::Group { items: second, .. } = &items[4] else {
        panic!()
    };
    assert_eq!(kinds(second), ["fill"]);
    let DrawItem::Redaction { rect, .. } = &items[1] else {
        panic!()
    };
    assert_eq!(rect.x0, 9.0);
}

#[test]
fn skipped_shapes_leave_the_list_including_their_highlight_holes() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let h = spotlight(0.0, 0.0);
    let d = doc(vec![a.clone(), h.clone()]);
    let skip = [a.id, h.id];
    let cx = BuildContext {
        glyphs: &FakeOutliner,
        skip: &skip,
    };
    assert!(build_draw_list(&d, &cx).items.is_empty());
}

#[test]
fn an_empty_document_has_an_empty_list() {
    assert!(list(&doc(vec![])).is_empty());
}

#[test]
fn per_shape_items_do_not_depend_on_the_other_shapes() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let alone = items_of(a.clone());
    let d = doc(vec![a.clone(), filled_rect(3.0, 3.0, 4.0, 4.0)]);
    let together = draw_items(&d, &a, &BuildContext::new(&FakeOutliner));
    assert_eq!(format!("{alone:?}"), format!("{together:?}"));
}
