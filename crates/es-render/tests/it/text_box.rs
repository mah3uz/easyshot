#![allow(clippy::unwrap_used)]

use es_anno::draw::DrawList;
use es_anno::{BuildContext, BundledText, Document, build_draw_list};
use es_geom::Affine;
use es_model::{AnnoShape, AnnotationDocument, ShapeKind, Swatch, TextBorder, TextProps};
use es_render::paint::{ClipShape, paint_draw_list};
use tiny_skia::{Color, Pixmap};

const W: u32 = 400;
const H: u32 = 300;
const WHITE: [u8; 3] = [255, 255, 255];

fn boxed(fill: Option<Swatch>, border: Option<TextBorder>, rotation: f64) -> (Pixmap, f64, f64) {
    let mut shape = AnnoShape::new(
        100.0,
        100.0,
        ShapeKind::Text(TextProps {
            text: "Hello World".into(),
            fill,
            border,
            ..TextProps::default()
        }),
    );
    shape.rotation = rotation;
    let doc = Document::new(
        AnnotationDocument {
            shapes: vec![shape],
            ..AnnotationDocument::default()
        },
        Box::new(BundledText::bundled()),
    );
    let text = BundledText::bundled();
    let list: DrawList = build_draw_list(&doc, &BuildContext::new(&text));
    let ShapeKind::Text(p) = &doc.shapes()[0].kind else {
        panic!("text");
    };
    let size = doc.measure_text(p);
    let mut out = Pixmap::new(W, H).unwrap();
    out.fill(Color::WHITE);
    let snapshot = out.clone();
    paint_draw_list(
        &list,
        &mut out,
        Affine::IDENTITY,
        ClipShape::page(f64::from(W), f64::from(H)),
        &snapshot,
    );
    (out, size.width, size.height)
}

fn px(p: &Pixmap, x: f64, y: f64) -> [u8; 3] {
    let c = p.pixel(x as u32, y as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

fn rgb(s: &Swatch) -> [u8; 3] {
    let c = s.to_rgba();
    [c.r, c.g, c.b].map(|v| (v * 255.0).round() as u8)
}

fn red_border() -> TextBorder {
    TextBorder {
        swatch: Swatch::red(),
        width: 4.0,
    }
}

#[test]
fn a_fill_paints_the_padding_around_the_text() {
    let (out, _, h) = boxed(Some(Swatch::blue()), None, 0.0);
    assert_eq!(px(&out, 96.0, 100.0 + h / 2.0), rgb(&Swatch::blue()));
    assert_eq!(px(&out, 80.0, 100.0 + h / 2.0), WHITE);
}

#[test]
fn no_fill_and_no_border_leave_the_padding_untouched() {
    let (out, _, h) = boxed(None, None, 0.0);
    assert_eq!(px(&out, 96.0, 100.0 + h / 2.0), WHITE);
}

#[test]
fn a_border_alone_leaves_the_inside_clear() {
    let (out, _, h) = boxed(None, Some(red_border()), 0.0);
    let mid = 100.0 + h / 2.0;
    assert_eq!(px(&out, 92.0, mid), rgb(&Swatch::red()));
    assert_eq!(px(&out, 97.0, mid), WHITE);
}

#[test]
fn the_box_turns_with_the_text() {
    let (flat, w, _) = boxed(Some(Swatch::blue()), None, 0.0);
    let (turned, _, _) = boxed(Some(Swatch::blue()), None, std::f64::consts::FRAC_PI_2);
    let (x, y) = (104.0, 100.0 + w / 2.0);
    assert_eq!(px(&flat, x, y), WHITE);
    assert_eq!(px(&turned, x, y), rgb(&Swatch::blue()));
}
