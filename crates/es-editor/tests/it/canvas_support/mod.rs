#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use es_anno::BundledText;
use es_editor::{EditorCanvas, EditorSession, Opened};
use es_geom::PVec;
use es_model::{
    AnnoShape, AnnotationDocument, ArrowProps, Arrowhead, BackgroundSettings, DashStyle, DrawProps,
    FillStyle, GeoKind, GeoProps, HighlightProps, NumberedProps, RedactionProps, ShapeKind, Swatch,
    TextProps,
};
use es_testkit::Headless;
use gpui::{AppContext, PlatformInput};
use image::{Rgba, RgbaImage};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

pub static TEXT: LazyLock<Arc<BundledText>> = LazyLock::new(|| Arc::new(BundledText::bundled()));

pub type App = Headless<EditorCanvas>;

/// A soft two-axis ramp with a grid, so misplaced pixels show.
pub fn base(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        if x % 100 == 0 || y % 100 == 0 {
            return Rgba([150, 160, 175, 255]);
        }
        let fx = x as f32 / width as f32;
        let fy = y as f32 / height as f32;
        Rgba([
            (235.0 - 50.0 * fx) as u8,
            (238.0 - 40.0 * fy) as u8,
            (245.0 - 20.0 * fx * fy) as u8,
            255,
        ])
    })
}

pub fn swatch(r: f64, g: f64, b: f64) -> Swatch {
    Swatch {
        red: r,
        green: g,
        blue: b,
        alpha: 1.0,
        ..Swatch::default()
    }
}

pub fn geo(x: f64, y: f64, props: GeoProps) -> AnnoShape {
    AnnoShape::new(x, y, ShapeKind::Geo(props))
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> AnnoShape {
    geo(
        x,
        y,
        GeoProps {
            w,
            h,
            swatch: swatch(0.9, 0.2, 0.15),
            stroke_width: 6.0,
            ..GeoProps::default()
        },
    )
}

pub fn wave(n: usize, x0: f64, y0: f64, len: f64) -> Vec<PVec> {
    (0..n)
        .map(|i| {
            let t = i as f64 / (n.max(2) - 1) as f64;
            PVec::new(x0 + t * len, y0 + (t * 12.0).sin() * 30.0)
        })
        .collect()
}

/// One of every kind the canvas paints, on a 1200 x 640 page; `redaction` adds a blur box.
pub fn every_kind(redaction: bool, text: bool) -> Vec<AnnoShape> {
    let mut shapes = vec![
        rect(40.0, 40.0, 220.0, 140.0),
        geo(
            300.0,
            40.0,
            GeoProps {
                w: 200.0,
                h: 140.0,
                swatch: swatch(0.1, 0.45, 0.9),
                stroke_width: 5.0,
                corner_radius: 24.0,
                fill: FillStyle::Solid,
                ..GeoProps::default()
            },
        ),
        geo(
            540.0,
            40.0,
            GeoProps {
                geo: GeoKind::Ellipse,
                w: 220.0,
                h: 140.0,
                swatch: swatch(0.1, 0.6, 0.3),
                stroke_width: 6.0,
                dash: DashStyle::Dashed,
                ..GeoProps::default()
            },
        ),
        AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Arrow(ArrowProps {
                start: PVec::new(60.0, 420.0),
                end: PVec::new(320.0, 240.0),
                arrowhead_end: Arrowhead::Arrow,
                swatch: swatch(0.95, 0.5, 0.1),
                stroke_width: 6.0,
                ..ArrowProps::default()
            }),
        ),
        AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Arrow(ArrowProps {
                start: PVec::new(380.0, 430.0),
                end: PVec::new(640.0, 260.0),
                bend: 60.0,
                arrowhead_start: Arrowhead::Dot,
                arrowhead_end: Arrowhead::Triangle,
                swatch: swatch(0.55, 0.2, 0.85),
                stroke_width: 4.0,
                dash: DashStyle::Dotted,
            }),
        ),
        AnnoShape::new(
            0.0,
            0.0,
            ShapeKind::Draw(DrawProps {
                points: wave(160, 60.0, 540.0, 560.0),
                is_complete: true,
                swatch: swatch(0.85, 0.1, 0.45),
                stroke_width: 6.0,
                ..DrawProps::default()
            }),
        ),
        AnnoShape::new(
            980.0,
            60.0,
            ShapeKind::Numbered(NumberedProps {
                value: 7,
                diameter: 64.0,
                swatch: swatch(0.95, 0.45, 0.1),
            }),
        ),
        AnnoShape::new(
            820.0,
            300.0,
            ShapeKind::Highlight(HighlightProps { w: 300.0, h: 160.0 }),
        ),
    ];
    if text {
        shapes.push(AnnoShape::new(
            700.0,
            520.0,
            ShapeKind::Text(TextProps {
                text: "Preview equals export".into(),
                font_size: 36.0,
                swatch: swatch(0.1, 0.1, 0.12),
                ..TextProps::default()
            }),
        ));
    }
    if redaction {
        shapes.push(AnnoShape::new(
            800.0,
            60.0,
            ShapeKind::Redaction(RedactionProps {
                w: 140.0,
                h: 80.0,
                ..RedactionProps::default()
            }),
        ));
    }
    shapes
}

pub fn opened(width: u32, height: u32, shapes: Vec<AnnoShape>, look: BackgroundSettings) -> Opened {
    Opened {
        display: "sample.png".into(),
        base: base(width, height),
        document: AnnotationDocument {
            shapes,
            background: look,
            ..AnnotationDocument::default()
        },
        read_only: false,
    }
}

pub fn open(width: f32, height: f32, doc: Opened) -> (App, EditorSession) {
    let slot: Rc<RefCell<Option<EditorSession>>> = Rc::default();
    let out = slot.clone();
    let app = Headless::open(
        es_ui::assets::Assets,
        es_ui::components::gallery::init,
        width,
        height,
        move |_, cx| {
            let session = EditorSession::new(doc, TEXT.clone(), cx);
            let canvas = cx.new(|cx| EditorCanvas::new(&session, cx));
            *out.borrow_mut() = Some(session);
            canvas
        },
    );
    let session = slot
        .borrow_mut()
        .take()
        .expect("the window built the session");
    (app, session)
}

pub fn canvas(app: &App) -> gpui::Entity<EditorCanvas> {
    app.window.entity(&app.cx).expect("the window has its root")
}

pub fn send(app: &mut App, event: PlatformInput) {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(event, cx);
        })
        .expect("window is open");
    app.settle();
}

/// The frame drawn again with every view re-rendered.
pub fn redraw(app: &mut App) {
    let canvas = canvas(app);
    app.cx.update(|cx| canvas.update(cx, |_, cx| cx.notify()));
    app.settle();
}
