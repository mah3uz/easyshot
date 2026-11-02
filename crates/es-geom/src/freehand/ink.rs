use super::options::StrokeOptions;
use super::pipeline::StrokePoint;
use super::tracks::{TrackPoint, build_tracks};
use kurbo::{BezPath, Point};

pub(crate) const CHUNK_POINTS: usize = 128;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Spec {
    a: usize,
    a_elbow: bool,
    inner_start: usize,
    b: usize,
    b_elbow: bool,
    dup_quirk: bool,
    len: usize,
    pub anchor: Option<Point>,
}

impl Spec {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get(&self, pts: &[StrokePoint], k: usize) -> TrackPoint {
        if k == 0 {
            let p = &pts[self.a];
            let mut t = TrackPoint::from_stroke(p, true);
            if self.a_elbow {
                t.p.x = p.input.x;
                t.p.y = p.input.y;
            }
            t.p.z = p.input.z;
            return t;
        }
        if k == self.len - 1 {
            let p = &pts[self.b];
            let mut t = TrackPoint::from_stroke(p, true);
            if self.b_elbow {
                t.p.x = p.input.x;
                t.p.y = p.input.y;
            }
            t.p.z = p.input.z;
            return t;
        }
        let p = &pts[self.inner_start + k - 1];
        let mut t = TrackPoint::from_stroke(p, self.dup_quirk && k == self.len - 2);
        t.p.z = p.input.z;
        t
    }
}

fn whole(n: usize) -> Spec {
    Spec {
        a: 0,
        a_elbow: false,
        inner_start: 1,
        b: n - 1,
        b_elbow: false,
        dup_quirk: false,
        len: n,
        anchor: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_partition(
    pts: &[StrokePoint],
    a: usize,
    a_elbow: bool,
    b: usize,
    b_elbow: bool,
    b_dup: bool,
    mut anchor: Option<Point>,
    out: &mut Vec<Spec>,
) {
    let len = b - a + 1 + usize::from(b_dup);
    let mut s = 0usize;
    let mut e = 0usize;

    let (sx, sy) = if a_elbow {
        (pts[a].input.x, pts[a].input.y)
    } else {
        (pts[a].point.x, pts[a].point.y)
    };
    let start_radius = pts[a].radius;
    while len - s > 2 {
        let i = a + 1 + s;
        let dx = sx - pts[i].point.x;
        let dy = sy - pts[i].point.y;
        let lim = ((start_radius + pts[i].radius) / 2.0) * 0.5;
        if dx * dx + dy * dy < lim * lim {
            anchor = Some(Point::new(pts[i].point.x, pts[i].point.y));
            s += 1;
        } else {
            break;
        }
    }

    let (ex, ey) = if b_elbow {
        (pts[b].input.x, pts[b].input.y)
    } else {
        (pts[b].point.x, pts[b].point.y)
    };
    let end_radius = pts[b].radius;
    while len - s - e > 2 {
        let i = if b_dup { b - e } else { b - 1 - e };
        let dx = ex - pts[i].point.x;
        let dy = ey - pts[i].point.y;
        let lim = ((end_radius + pts[i].radius) / 2.0) * 0.5;
        if dx * dx + dy * dy < lim * lim {
            e += 1;
        } else {
            break;
        }
    }

    let inner_start = a + 1 + s;
    let inner_end_excl = if b_dup { b - e + 1 } else { b - e };
    let inner = inner_end_excl.saturating_sub(inner_start);
    out.push(Spec {
        a,
        a_elbow,
        inner_start,
        b,
        b_elbow,
        dup_quirk: b_dup && e == 0,
        len: inner + 2,
        anchor,
    });
}

pub(crate) fn partitions(pts: &[StrokePoint], out: &mut Vec<Spec>) {
    out.clear();
    let n = pts.len();
    if n == 0 {
        return;
    }
    if n <= 2 {
        out.push(whole(n));
        return;
    }

    let mut a = 0;
    let mut a_elbow = false;
    let mut anchor: Option<Point> = None;

    let dir = |i: usize, j: usize| {
        let dx = pts[j].point.x - pts[i].point.x;
        let dy = pts[j].point.y - pts[i].point.y;
        let l = (dx * dx + dy * dy).sqrt();
        (dx / l, dy / l)
    };
    let mut prev = dir(0, 1);

    for i in 1..n - 1 {
        let next = dir(i, i + 1);
        let dpr = prev.0 * next.0 + prev.1 * next.1;
        prev = next;

        if dpr < -0.8 {
            finish_partition(pts, a, a_elbow, i, true, false, anchor, out);
            a = i;
            a_elbow = true;
            anchor = Some(Point::new(pts[i].point.x, pts[i].point.y));
            continue;
        }
        if dpr > 0.7 {
            continue;
        }
        let pdx = pts[i].point.x - pts[i - 1].point.x;
        let pdy = pts[i].point.y - pts[i - 1].point.y;
        let ndx = pts[i + 1].point.x - pts[i].point.x;
        let ndy = pts[i + 1].point.y - pts[i].point.y;
        let mean = (pts[i - 1].radius + pts[i].radius + pts[i + 1].radius) / 3.0;
        if (pdx * pdx + pdy * pdy + ndx * ndx + ndy * ndy) / (mean * mean) < 1.5 {
            finish_partition(pts, a, a_elbow, i, false, true, anchor, out);
            a = i;
            a_elbow = false;
            anchor = None;
        }
    }
    finish_partition(pts, a, a_elbow, n - 1, false, false, anchor, out);
}

pub(crate) fn chunk_ranges(len: usize, mut f: impl FnMut(usize, usize)) {
    if len <= CHUNK_POINTS {
        f(0, len - 1);
        return;
    }
    let mut start = 0;
    while start < len - 1 {
        let end = (start + CHUNK_POINTS - 1).min(len - 1);
        f(start, end);
        start = end;
    }
}

struct Sink {
    path: BezPath,
    current: Point,
    last_control: Option<Point>,
}

impl Sink {
    fn new() -> Self {
        Self {
            path: BezPath::new(),
            current: Point::ZERO,
            last_control: None,
        }
    }

    fn move_to(&mut self, p: Point) {
        self.path.move_to(p);
        self.current = p;
        self.last_control = None;
    }

    fn smooth_quad(&mut self, p: Point) {
        let c = match self.last_control {
            Some(l) => Point::new(2.0 * self.current.x - l.x, 2.0 * self.current.y - l.y),
            None => self.current,
        };
        self.path.quad_to(c, p);
        self.last_control = Some(c);
        self.current = p;
    }

    fn break_run(&mut self) {
        self.last_control = None;
    }

    fn arc(&mut self, radius: f64, to: Point) {
        let from = self.current;
        if from != to {
            semicircle(&mut self.path, from, to, radius);
        }
        self.current = to;
        self.last_control = None;
    }

    fn close(&mut self) {
        self.path.close_path();
        self.last_control = None;
    }

    fn circle(&mut self, c: Point, r: f64) {
        const K: f64 = 0.552_284_749_830_793_4;
        let k = r * K;
        self.path.move_to((c.x + r, c.y));
        self.path
            .curve_to((c.x + r, c.y + k), (c.x + k, c.y + r), (c.x, c.y + r));
        self.path
            .curve_to((c.x - k, c.y + r), (c.x - r, c.y + k), (c.x - r, c.y));
        self.path
            .curve_to((c.x - r, c.y - k), (c.x - k, c.y - r), (c.x, c.y - r));
        self.path
            .curve_to((c.x + k, c.y - r), (c.x + r, c.y - k), (c.x + r, c.y));
        self.path.close_path();
    }
}

// Circular arc, large-arc 0, sweep 1, as up to four cubics.
fn semicircle(path: &mut BezPath, p1: Point, p2: Point, radius: f64) {
    use std::f64::consts::TAU;
    let mut r = radius.abs();
    if r == 0.0 {
        path.line_to(p2);
        return;
    }
    let dx = (p1.x - p2.x) / 2.0;
    let dy = (p1.y - p2.y) / 2.0;
    let lambda = (dx * dx + dy * dy) / (r * r);
    if lambda > 1.0 {
        r *= lambda.sqrt();
    }
    let numerator = r * r * dy * dy + r * r * dx * dx;
    let term = r * r * r * r - r * r * dy * dy - r * r * dx * dx;
    let radicand = if numerator == 0.0 {
        0.0
    } else {
        term / numerator
    };
    let coef = radicand.max(0.0).sqrt();
    let cxp = coef * ((r * dy) / r);
    let cyp = coef * (-(r * dx) / r);
    let cx = cxp + (p1.x + p2.x) / 2.0;
    let cy = cyp + (p1.y + p2.y) / 2.0;
    let start = ((dy - cyp) / r).atan2((dx - cxp) / r);
    let mut end = ((-dy - cyp) / r).atan2((-dx - cxp) / r);
    if end < start {
        end += TAU;
    }
    let sweep = end - start;
    let segments = ((sweep.abs() / std::f64::consts::FRAC_PI_2).ceil() as usize).clamp(1, 4);
    let per = sweep / segments as f64;
    let at = |a: f64| Point::new(cx + r * a.cos(), cy + r * a.sin());
    let d = |a: f64| Point::new(-r * a.sin(), r * a.cos());
    let handle = (4.0 / 3.0) * (per / 4.0).tan();
    for i in 0..segments {
        let t1 = start + i as f64 * per;
        let t2 = start + (i + 1) as f64 * per;
        let (s, e, d1, d2) = (at(t1), at(t2), d(t1), d(t2));
        path.curve_to(
            Point::new(s.x + handle * d1.x, s.y + handle * d1.y),
            Point::new(e.x - handle * d2.x, e.y - handle * d2.y),
            e,
        );
    }
}

pub(crate) fn render_partition(
    src: &[TrackPoint],
    opts: &StrokeOptions,
    anchor: Option<Point>,
) -> BezPath {
    let n = src.len();
    let mut sink = Sink::new();
    if n == 0 {
        return sink.path;
    }
    if n == 1 {
        sink.circle(Point::new(src[0].p.x, src[0].p.y), src[0].radius);
        return sink.path;
    }
    let tracks = build_tracks(src, opts, anchor);
    if tracks.left.is_empty() || tracks.right.is_empty() {
        return sink.path;
    }

    sink.move_to(tracks.left[0]);
    let mut prev = tracks.left[0];
    for &pt in &tracks.left[1..] {
        sink.smooth_quad(prev.midpoint(pt));
        prev = pt;
    }

    let cap_offset = |from: Point, to: Point, radius: f64, sign: f64| {
        let vx = from.x - to.x;
        let vy = from.y - to.y;
        let l = (vx * vx + vy * vy).sqrt();
        if l == 0.0 {
            return (0.0, 0.0);
        }
        (sign * (-vy / l) * radius, sign * (vx / l) * radius)
    };

    {
        let point = Point::new(src[n - 1].p.x, src[n - 1].p.y);
        let radius = src[n - 1].radius;
        let before = Point::new(src[n - 2].p.x, src[n - 2].p.y);
        let (dx, dy) = cap_offset(before, point, radius, 1.0);
        sink.smooth_quad(Point::new(point.x + dx, point.y + dy));
        sink.arc(radius, Point::new(point.x - dx, point.y - dy));
        sink.break_run();
    }

    prev = tracks.right[tracks.right.len() - 1];
    for &pt in tracks.right.iter().rev().skip(1) {
        sink.smooth_quad(prev.midpoint(pt));
        prev = pt;
    }

    {
        let point = Point::new(src[0].p.x, src[0].p.y);
        let radius = src[0].radius;
        let next = Point::new(src[1].p.x, src[1].p.y);
        let (dx, dy) = cap_offset(point, next, radius, -1.0);
        sink.smooth_quad(Point::new(point.x + dx, point.y + dy));
        sink.arc(radius, Point::new(point.x - dx, point.y - dy));
        sink.close();
    }
    sink.path
}
