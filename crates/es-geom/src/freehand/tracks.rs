use super::options::StrokeOptions;
use super::pipeline::StrokePoint;
use crate::vec::PVec;
use kurbo::Point;

pub(crate) const FIXED_PI: f64 = std::f64::consts::PI + 0.0001;
const TRACK_TOLERANCE_RATIO: f64 = 0.05;
const SIMPLIFY_WINDOW: usize = 8;
pub(crate) const MIN_ROUNDED_CORNER_STEPS: usize = 8;
pub(crate) const MAX_ROUNDED_CORNER_STEPS: usize = 13;
const HARD_CORNER_DPR: f64 = -0.62;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TrackPoint {
    pub p: PVec,
    pub input: PVec,
    pub radius: f64,
    pub running: f64,
    pub is_cap: bool,
}

impl TrackPoint {
    pub fn from_stroke(sp: &StrokePoint, is_cap: bool) -> Self {
        Self {
            p: sp.point,
            input: sp.input,
            radius: sp.radius,
            running: sp.running_length,
            is_cap,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Tracks {
    pub left: Vec<Point>,
    pub right: Vec<Point>,
}

fn simplify_track(pts: &mut Vec<Point>, tol: f64) {
    let len = pts.len();
    if len <= 2 || tol <= 0.0 {
        return;
    }
    let tol2 = tol * tol;
    let mut out = 1;
    let mut anchor = 0;
    let last = len - 1;
    while anchor < last {
        let mut best = anchor + 1;
        let max_j = (anchor + SIMPLIFY_WINDOW).min(last);
        let a = pts[anchor];
        if anchor + 2 <= max_j {
            'outer: for j in (anchor + 2)..=max_j {
                let acx = pts[j].x - a.x;
                let acy = pts[j].y - a.y;
                let l2 = acx * acx + acy * acy;
                for pk in &pts[anchor + 1..j] {
                    let mut t = if l2 == 0.0 {
                        0.0
                    } else {
                        ((pk.x - a.x) * acx + (pk.y - a.y) * acy) / l2
                    };
                    t = t.clamp(0.0, 1.0);
                    let ex = pk.x - (a.x + acx * t);
                    let ey = pk.y - (a.y + acy * t);
                    if ex * ex + ey * ey > tol2 {
                        break 'outer;
                    }
                }
                best = j;
            }
        }
        pts[out] = pts[best];
        out += 1;
        anchor = best;
    }
    pts.truncate(out);
}

fn unit_back(from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let dx = from.0 - to.0;
    let dy = from.1 - to.1;
    let l = (dx * dx + dy * dy).sqrt();
    if l == 0.0 { (dx, dy) } else { (dx / l, dy / l) }
}

pub(crate) fn build_tracks(
    src: &[TrackPoint],
    opts: &StrokeOptions,
    anchor: Option<Point>,
) -> Tracks {
    let n = src.len();
    let size = opts.size;
    if n == 0 || size <= 0.0 {
        return Tracks::default();
    }
    let mut left = Vec::with_capacity(n + 4);
    let mut right = Vec::with_capacity(n + 4);
    let total_length = src[n - 1].running;
    let min_distance = (size * opts.smoothing).powi(2);

    let mut cur_vec = (1.0, 1.0);
    if n > 1 {
        cur_vec = unit_back((src[0].p.x, src[0].p.y), (src[1].p.x, src[1].p.y));
    }
    let mut prev_vec = cur_vec;

    let (mut plx, mut ply) = (src[0].p.x, src[0].p.y);
    let (mut prx, mut pry) = (plx, ply);
    let (mut tlx, mut tly) = (plx, ply);
    let (mut trx, mut try_) = (prx, pry);
    let mut prev_was_sharp = false;

    for i in 0..n {
        let s = &src[i];
        let (px, py) = (s.p.x, s.p.y);
        let radius = s.radius;
        let vec = cur_vec;

        let mut next_vec = vec;
        if i < n - 1 {
            let from = match anchor {
                Some(a) if i == 0 && n > 2 => (a.x, a.y),
                _ => (px, py),
            };
            next_vec = unit_back(from, (src[i + 1].p.x, src[i + 1].p.y));
        }
        cur_vec = next_vec;

        let prev_dpr = vec.0 * prev_vec.0 + vec.1 * prev_vec.1;
        let next_dpr = if i < n - 1 {
            next_vec.0 * vec.0 + next_vec.1 * vec.1
        } else {
            1.0
        };

        let is_point_sharp = prev_dpr < 0.0 && !prev_was_sharp;
        let is_next_sharp = next_dpr < 0.2;

        if is_point_sharp || is_next_sharp {
            if next_dpr > HARD_CORNER_DPR && total_length - s.running > radius {
                let ox = prev_vec.0 * radius;
                let oy = prev_vec.1 * radius;
                let cpr = prev_vec.0 * next_vec.1 - prev_vec.1 * next_vec.0;
                if cpr < 0.0 {
                    (tlx, tly, trx, try_) = (px + ox, py + oy, px - ox, py - oy);
                } else {
                    (tlx, tly, trx, try_) = (px - ox, py - oy, px + ox, py + oy);
                }
                left.push(Point::new(tlx, tly));
                right.push(Point::new(trx, try_));
            } else {
                let (inx, iny) = (s.input.x, s.input.y);
                let dx = -prev_vec.1 * radius;
                let dy = prev_vec.0 * radius;
                let step = 1.0 / MAX_ROUNDED_CORNER_STEPS as f64;
                let mut t = 0.0;
                while t < 1.0 {
                    let (sn, cs) = (FIXED_PI * t).sin_cos();
                    tlx = inx + (dx * cs - dy * sn);
                    tly = iny + (dx * sn + dy * cs);
                    left.push(Point::new(tlx, tly));
                    let (sn, cs) = (FIXED_PI + FIXED_PI * -t).sin_cos();
                    trx = inx + (dx * cs - dy * sn);
                    try_ = iny + (dx * sn + dy * cs);
                    right.push(Point::new(trx, try_));
                    t += step;
                }
            }
            (plx, ply, prx, pry) = (tlx, tly, trx, try_);
            if is_next_sharp {
                prev_was_sharp = true;
            }
            continue;
        }

        prev_was_sharp = false;

        if s.is_cap {
            let ox = vec.1 * radius;
            let oy = -vec.0 * radius;
            left.push(Point::new(px - ox, py - oy));
            right.push(Point::new(px + ox, py + oy));
            continue;
        }

        let lx = next_vec.0 + (vec.0 - next_vec.0) * next_dpr;
        let ly = next_vec.1 + (vec.1 - next_vec.1) * next_dpr;
        let ox = ly * radius;
        let oy = -lx * radius;

        tlx = px - ox;
        tly = py - oy;
        if i <= 1 || (plx - tlx).powi(2) + (ply - tly).powi(2) > min_distance {
            left.push(Point::new(tlx, tly));
            plx = tlx;
            ply = tly;
        }
        trx = px + ox;
        try_ = py + oy;
        if i <= 1 || (prx - trx).powi(2) + (pry - try_).powi(2) > min_distance {
            right.push(Point::new(trx, try_));
            prx = trx;
            pry = try_;
        }
        prev_vec = vec;
    }

    let tol = size * TRACK_TOLERANCE_RATIO;
    simplify_track(&mut left, tol);
    simplify_track(&mut right, tol);
    Tracks { left, right }
}
