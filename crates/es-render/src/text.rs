use es_geom::BezPath;
use kurbo::PathEl;
use tiny_skia::{Path, PathBuilder};

pub fn skia_path(path: &BezPath) -> Option<Path> {
    let mut b = PathBuilder::new();
    let f = |p: kurbo::Point| (p.x as f32, p.y as f32);
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                let (x, y) = f(p);
                b.move_to(x, y);
            }
            PathEl::LineTo(p) => {
                let (x, y) = f(p);
                b.line_to(x, y);
            }
            PathEl::QuadTo(c, p) => {
                let (cx, cy) = f(c);
                let (x, y) = f(p);
                b.quad_to(cx, cy, x, y);
            }
            PathEl::CurveTo(c1, c2, p) => {
                let (x1, y1) = f(c1);
                let (x2, y2) = f(c2);
                let (x, y) = f(p);
                b.cubic_to(x1, y1, x2, y2, x, y);
            }
            PathEl::ClosePath => b.close(),
        }
    }
    b.finish()
}
