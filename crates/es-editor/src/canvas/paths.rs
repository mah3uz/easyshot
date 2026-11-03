use es_anno::{DrawItem, Viewport};
use es_geom::{Affine, BezPath, Cap, FillRule, Join, Rect, Rgba};
use gpui::{Bounds, FillOptions, Hsla, PathBuilder, PathStyle, Pixels, Point, point, px};
use kurbo::{PathEl, Shape};

/// Device pixels a flattened curve may stray from the true one.
pub const DEVICE_TOLERANCE: f64 = 0.25;
/// Longer centrelines are stroked in overlapping runs, keeping each GPU path well under its vertex cap.
pub const STROKE_RUN: usize = 128;

pub type GpuPath = gpui::Path<Pixels>;

#[derive(Clone)]
pub struct Painted {
    pub path: GpuPath,
    pub color: Hsla,
}

/// Paths are tessellated at the top of a zoom bucket, so placing them only ever shrinks them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bucket {
    pub index: i32,
    pub scale_factor: f32,
}

impl Bucket {
    pub fn for_scale(scale: f64, scale_factor: f32) -> Self {
        let s = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        Self {
            index: (s.log2() * 2.0).floor() as i32,
            scale_factor,
        }
    }

    /// Logical pixels per page pixel the bucket is built at.
    pub fn scale(&self) -> f64 {
        2f64.powf(f64::from(self.index + 1) / 2.0)
    }

    fn tolerance(&self) -> f32 {
        (DEVICE_TOLERANCE / f64::from(self.scale_factor.max(1.0))) as f32
    }
}

/// Where bucket-space paths land in the window: `window = offset + bucket_point * k`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub offset: Point<Pixels>,
    pub k: f32,
}

impl Placement {
    pub fn new(element_origin: Point<Pixels>, viewport: &Viewport, bucket: Bucket) -> Self {
        Self {
            offset: element_origin
                + point(px(viewport.origin.x as f32), px(viewport.origin.y as f32)),
            k: (viewport.scale / bucket.scale()) as f32,
        }
    }

    pub fn place(&self, p: &GpuPath) -> GpuPath {
        let mut out = p.clone();
        let (k, o) = (self.k, self.offset);
        let map = |q: Point<Pixels>| point(q.x * k + o.x, q.y * k + o.y);
        for v in &mut out.vertices {
            v.xy_position = map(v.xy_position);
        }
        out.bounds = Bounds::new(
            map(p.bounds.origin),
            gpui::size(p.bounds.size.width * k, p.bounds.size.height * k),
        );
        out
    }
}

fn hsla(c: Rgba, opacity: f64) -> Hsla {
    es_ui::convert::hsla(Rgba::new(
        c.r,
        c.g,
        c.b,
        (f64::from(c.a) * opacity).clamp(0.0, 1.0) as f32,
    ))
}

/// Tessellates one shape's draw items in bucket space; redactions and spotlight holes are skipped.
pub fn tessellate(items: &[DrawItem], bucket: Bucket) -> Vec<Painted> {
    let mut out = Vec::new();
    let to_bucket = Affine::scale(bucket.scale());
    walk(items, to_bucket, 1.0, bucket, &mut out);
    out
}

fn walk(items: &[DrawItem], ts: Affine, opacity: f64, bucket: Bucket, out: &mut Vec<Painted>) {
    for item in items {
        match item {
            DrawItem::Fill { path, color, rule } => {
                fill(path, ts, *rule, hsla(*color, opacity), bucket, out);
            }
            DrawItem::Stroke {
                path,
                color,
                width,
                cap,
                join,
                ..
            } => {
                let local_tol = f64::from(bucket.tolerance()) / affine_scale(ts).max(1e-9);
                let style = kurbo::Stroke::new(*width)
                    .with_caps(match cap {
                        Cap::Butt => kurbo::Cap::Butt,
                        Cap::Round => kurbo::Cap::Round,
                        Cap::Square => kurbo::Cap::Square,
                    })
                    .with_join(match join {
                        Join::Miter => kurbo::Join::Miter,
                        Join::Round => kurbo::Join::Round,
                        Join::Bevel => kurbo::Join::Bevel,
                    });
                let color = hsla(*color, opacity);
                for run in runs(path, STROKE_RUN) {
                    let outline =
                        kurbo::stroke(run, &style, &kurbo::StrokeOpts::default(), local_tol);
                    fill(&outline, ts, FillRule::NonZero, color, bucket, out);
                }
            }
            DrawItem::Text {
                glyphs,
                color,
                underline,
            } => {
                let color = hsla(*color, opacity);
                fill(glyphs, ts, FillRule::NonZero, color, bucket, out);
                for r in underline {
                    fill(&r.to_path(0.1), ts, FillRule::NonZero, color, bucket, out);
                }
            }
            DrawItem::Group {
                transform,
                opacity: o,
                items,
            } => walk(items, ts * *transform, opacity * o, bucket, out),
            DrawItem::Redaction { .. }
            | DrawItem::Spotlight { .. }
            | DrawItem::Highlighter { .. } => {}
        }
    }
}

fn affine_scale(a: Affine) -> f64 {
    a.determinant().abs().sqrt()
}

/// Splits a long centreline into runs that share their end points, so round joins stay seamless.
fn runs(path: &BezPath, max: usize) -> Vec<BezPath> {
    let els = path.elements();
    if els.len() <= max {
        return vec![path.clone()];
    }
    let mut out = Vec::new();
    let mut current = BezPath::new();
    let mut count = 0;
    let mut last = kurbo::Point::ZERO;
    for el in els {
        if count >= max && !matches!(el, PathEl::MoveTo(_) | PathEl::ClosePath) {
            out.push(std::mem::take(&mut current));
            current.move_to(last);
            count = 0;
        }
        current.push(*el);
        count += 1;
        if let Some(p) = el.end_point() {
            last = p;
        }
    }
    if !current.elements().is_empty() {
        out.push(current);
    }
    out
}

fn gp(p: kurbo::Point) -> Point<Pixels> {
    point(px(p.x as f32), px(p.y as f32))
}

fn builder(rule: FillRule, bucket: Bucket) -> PathBuilder {
    let rule = match rule {
        FillRule::NonZero => gpui::FillRule::NonZero,
        FillRule::EvenOdd => gpui::FillRule::EvenOdd,
    };
    PathBuilder::fill().with_style(PathStyle::Fill(
        FillOptions::default()
            .with_fill_rule(rule)
            .with_tolerance(bucket.tolerance()),
    ))
}

/// One GPU path per run of subpaths; the vertex cap splits a path that tessellates too large.
pub fn fill(
    path: &BezPath,
    ts: Affine,
    rule: FillRule,
    color: Hsla,
    bucket: Bucket,
    out: &mut Vec<Painted>,
) {
    let subpaths = split_subpaths(path);
    if subpaths.is_empty() {
        return;
    }
    build_groups(&subpaths, ts, rule, color, bucket, out);
}

fn build_groups(
    subpaths: &[&[PathEl]],
    ts: Affine,
    rule: FillRule,
    color: Hsla,
    bucket: Bucket,
    out: &mut Vec<Painted>,
) {
    let mut pb = builder(rule, bucket);
    for sub in subpaths {
        append(&mut pb, sub, ts);
    }
    match pb.build() {
        Ok(path) if !path.vertices.is_empty() => out.push(Painted { path, color }),
        Ok(_) => {}
        Err(error) if subpaths.len() > 1 => {
            tracing::debug!(%error, "splitting a path that tessellates too large");
            let (a, b) = subpaths.split_at(subpaths.len() / 2);
            build_groups(a, ts, rule, color, bucket, out);
            build_groups(b, ts, rule, color, bucket, out);
        }
        Err(error) => tracing::warn!(%error, "a shape outline could not be tessellated"),
    }
}

fn split_subpaths(path: &BezPath) -> Vec<&[PathEl]> {
    let els = path.elements();
    let mut out = Vec::new();
    let mut start = 0;
    for (i, el) in els.iter().enumerate() {
        if matches!(el, PathEl::MoveTo(_)) && i > start {
            out.push(&els[start..i]);
            start = i;
        }
    }
    if start < els.len() {
        out.push(&els[start..]);
    }
    out
}

fn append(pb: &mut PathBuilder, els: &[PathEl], ts: Affine) {
    let mut open = false;
    for el in els {
        match *el {
            PathEl::MoveTo(p) => {
                if open {
                    pb.close();
                }
                pb.move_to(gp(ts * p));
                open = true;
            }
            PathEl::LineTo(p) => pb.line_to(gp(ts * p)),
            PathEl::QuadTo(c, p) => pb.curve_to(gp(ts * p), gp(ts * c)),
            PathEl::CurveTo(c1, c2, p) => pb.cubic_bezier_to(gp(ts * p), gp(ts * c1), gp(ts * c2)),
            PathEl::ClosePath => {
                pb.close();
                open = false;
            }
        }
    }
    if open {
        pb.close();
    }
}

/// The dim layer: the screenshot's outline with the spotlight holes cut out of it.
pub fn spotlight(
    clip: Rect,
    radii: [f64; 4],
    holes: &[BezPath],
    bucket: Bucket,
    color: Hsla,
) -> Vec<Painted> {
    let mut path = kurbo::RoundedRect::from_rect(
        clip,
        kurbo::RoundedRectRadii::new(radii[0], radii[1], radii[2], radii[3]),
    )
    .to_path(0.05);
    for hole in holes {
        path.extend(hole.elements().iter().copied());
    }
    let mut pb = builder(FillRule::EvenOdd, bucket);
    append(&mut pb, path.elements(), Affine::scale(bucket.scale()));
    match pb.build() {
        Ok(path) => vec![Painted { path, color }],
        Err(error) => {
            tracing::warn!(%error, "the spotlight could not be tessellated");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_never_magnifies_its_paths() {
        for s in [0.05, 0.3, 0.5, 0.99, 1.0, 1.7, 4.0] {
            let b = Bucket::for_scale(s, 2.0);
            let k = s / b.scale();
            assert!(k <= 1.0 && k > 0.7, "scale {s} placed at k = {k}");
        }
    }

    #[test]
    fn long_centrelines_are_split_into_joined_runs() {
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        for i in 1..=300 {
            path.line_to((f64::from(i), 0.0));
        }
        let parts = runs(&path, 128);
        assert_eq!(parts.len(), 3);
        for pair in parts.windows(2) {
            let end = pair[0].elements().last().and_then(|e| e.end_point());
            let start = pair[1].elements().first().and_then(|e| e.end_point());
            assert_eq!(end, start);
        }
    }
}
