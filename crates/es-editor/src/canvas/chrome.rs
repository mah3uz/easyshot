use super::EditorCanvas;
use super::paths::{self, Bucket, Painted};
use es_anno::document::page_transform;
use es_anno::{Editor, Viewport};
use es_geom::{Affine, BezPath, FillRule, PVec, Point, Rgba};
use es_model::Tool;
use es_ui::tokens::Appearance;
use gpui::{App, Bounds, Hsla, Pixels};
use kurbo::Shape;

const FRAME_WIDTH: f64 = 1.5;
const HANDLE_SIZE: f64 = 8.0;
const HANDLE_BORDER: f64 = 1.0;
const GUIDE_WIDTH: f64 = 1.0;
const GUIDE_DASH: [f64; 2] = [4.0, 4.0];
const GUIDE_ALPHA: f32 = 0.5;

struct Colours {
    line: Hsla,
    hint: Hsla,
    handle: Hsla,
    fill: Hsla,
}

fn rgb(hex: u32, alpha: f32) -> Hsla {
    let c = |shift: u32| ((hex >> shift) & 0xff) as f32 / 255.0;
    es_ui::convert::hsla(Rgba {
        r: c(16),
        g: c(8),
        b: c(0),
        a: alpha,
    })
}

impl Colours {
    fn new(cx: &App) -> Self {
        let t = es_ui::theme::tokens(cx);
        let (handle, fill) = match t.appearance {
            Appearance::Light => (rgb(0xffffff, 1.0), rgb(0x1f8fff, 0.24)),
            Appearance::Dark => (rgb(0x2a2a2c, 1.0), rgb(0x1f91ff, 0.20)),
        };
        Self {
            line: t.selection,
            hint: t.accent,
            handle,
            fill,
        }
    }
}

struct Pen {
    to_window: Affine,
    bucket: Bucket,
    out: Vec<Painted>,
}

impl Pen {
    fn fill(&mut self, path: &BezPath, color: Hsla) {
        paths::fill(
            path,
            self.to_window,
            FillRule::NonZero,
            color,
            self.bucket,
            &mut self.out,
        );
    }

    fn stroke(&mut self, path: &BezPath, width: f64, dash: Option<&[f64]>, color: Hsla) {
        let mut style = kurbo::Stroke::new(width);
        if let Some(dash) = dash {
            style = style.with_dashes(0.0, dash.iter().copied());
        }
        let outline = kurbo::stroke(path, &style, &kurbo::StrokeOpts::default(), 0.05);
        self.fill(&outline, color);
    }

    fn outlined(&mut self, path: &BezPath, fill: Hsla, line: Hsla, width: f64) {
        self.fill(path, fill);
        self.stroke(path, width, None, line);
    }
}

fn quad(points: [Point; 4]) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(points[0]);
    for p in &points[1..] {
        path.line_to(*p);
    }
    path.close_path();
    path
}

fn screen(vp: &Viewport, p: PVec) -> Point {
    vp.page_to_screen(Point::new(p.x, p.y))
}

/// A square handle turned with the frame's top edge, so it sits square on a rotated selection.
fn square(at: Point, along: kurbo::Vec2) -> BezPath {
    let u = along * (HANDLE_SIZE / 2.0);
    let v = kurbo::Vec2::new(-u.y, u.x);
    quad([at - u - v, at + u - v, at + u + v, at - u + v])
}

fn dot(at: Point) -> BezPath {
    kurbo::Circle::new(at, HANDLE_SIZE / 2.0).to_path(0.05)
}

impl EditorCanvas {
    /// Selection frame, handles, marquee and binding hint, in window pixels at a constant on-screen size.
    pub(super) fn chrome(
        &self,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
        cx: &App,
    ) -> Vec<Painted> {
        let editor = self.doc.read(cx).editor();
        let vp = self.viewport.read(cx).viewport();
        let colours = Colours::new(cx);
        let mut pen = Pen {
            to_window: Affine::translate((f64::from(bounds.origin.x), f64::from(bounds.origin.y))),
            bucket: Bucket {
                index: 0,
                scale_factor,
            },
            out: Vec::new(),
        };
        binding_hint(editor, &vp, &colours, &mut pen);
        if let Some(brush) = editor.brush() {
            let a = vp.page_to_screen(Point::new(brush.x0, brush.y0));
            let b = vp.page_to_screen(Point::new(brush.x1, brush.y1));
            let rect = kurbo::Rect::from_points(a, b).to_path(0.05);
            pen.outlined(&rect, colours.fill, colours.line, FRAME_WIDTH);
        }
        if editor.tool() == Tool::Select
            && editor.editing_text_id().is_none()
            && !arrow_handles(editor, &vp, &colours, &mut pen)
        {
            selection_frame(editor, &vp, &colours, &mut pen);
        }
        pen.out
    }
}

fn binding_hint(editor: &Editor, vp: &Viewport, colours: &Colours, pen: &mut Pen) {
    let doc = editor.doc();
    let Some((shape, geometry)) = editor
        .hinted_binding_id()
        .and_then(|id| Some((doc.shape(id)?, doc.geometry(id)?)))
    else {
        return;
    };
    let t = page_transform(shape);
    let corners = geometry
        .bounds()
        .corners()
        .map(|c| screen(vp, t.apply_to_point(c)));
    pen.outlined(&quad(corners), colours.fill, colours.hint, FRAME_WIDTH);
}

/// A lone arrow shows its three handles on a dashed guide instead of a frame.
fn arrow_handles(editor: &Editor, vp: &Viewport, colours: &Colours, pen: &mut Pen) -> bool {
    let doc = editor.doc();
    let ids = editor.selected_ids();
    let Some(shape) = ids
        .first()
        .filter(|_| ids.len() == 1)
        .and_then(|id| doc.shape(*id))
        .filter(|s| s.is_arrow())
    else {
        return false;
    };
    let Some(info) = doc.arrow_info(shape.id) else {
        return true;
    };
    let t = page_transform(shape);
    let [start, middle, end] = info.handles().map(|h| screen(vp, t.apply_to_point(h)));
    let mut guide = BezPath::new();
    guide.move_to(start);
    guide.quad_to(
        Point::new(
            2.0 * middle.x - (start.x + end.x) / 2.0,
            2.0 * middle.y - (start.y + end.y) / 2.0,
        ),
        end,
    );
    let mut line = colours.line;
    line.a *= GUIDE_ALPHA;
    pen.stroke(&guide, GUIDE_WIDTH, Some(&GUIDE_DASH), line);
    for at in [start, middle, end] {
        pen.outlined(&dot(at), colours.handle, colours.line, HANDLE_BORDER);
    }
    true
}

/// Only the corner squares are drawn; edge and rotate handles are live but invisible.
fn selection_frame(editor: &Editor, vp: &Viewport, colours: &Colours, pen: &mut Pen) {
    let Some(bounds) = editor.selection_bounds() else {
        return;
    };
    let corners = bounds.page_corners().map(|c| screen(vp, c));
    pen.stroke(&quad(corners), FRAME_WIDTH, None, colours.line);
    let top = corners[1] - corners[0];
    let along = if top.hypot() > f64::EPSILON {
        top.normalize()
    } else {
        kurbo::Vec2::new(1.0, 0.0)
    };
    for at in corners {
        pen.outlined(
            &square(at, along),
            colours.handle,
            colours.line,
            HANDLE_BORDER,
        );
    }
}
