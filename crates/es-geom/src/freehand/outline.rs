use super::options::{StrokeOptions, resolve_taper};
use super::pipeline::StrokePoint;
use super::tracks::{
    FIXED_PI, MAX_ROUNDED_CORNER_STEPS, MIN_ROUNDED_CORNER_STEPS, TrackPoint, build_tracks,
};
use crate::vec::PVec;

const MIN_CAP_STEPS: usize = 8;
const MAX_CAP_STEPS: usize = 29;

fn arc_steps(radius: f64, sweep: f64, tol: f64, min: usize, max: usize) -> usize {
    if radius <= tol {
        return min;
    }
    let max_angle = 2.0 * (1.0 - tol / radius).acos();
    let steps = (sweep / max_angle).ceil() as usize;
    steps.clamp(min, max)
}

fn pt(p: kurbo::Point) -> PVec {
    PVec::new(p.x, p.y)
}

pub(crate) fn outline(points: &[StrokePoint], opts: &StrokeOptions) -> Vec<PVec> {
    let n = points.len();
    let size = opts.size;
    if n == 0 || size <= 0.0 {
        return Vec::new();
    }
    let src: Vec<TrackPoint> = points
        .iter()
        .enumerate()
        .map(|(i, p)| TrackPoint::from_stroke(p, i == 0 || i == n - 1))
        .collect();

    let total = src[n - 1].running;
    let taper_start = resolve_taper(opts.start.taper, size, total);
    let taper_end = resolve_taper(opts.end.taper, size, total);
    let tracks = build_tracks(&src, opts, None);
    let cap_tol = (size * 0.02).max(0.05);

    let first_radius = src[0].radius;
    let first = src[0].p;
    let last = if n > 1 {
        src[n - 1].p
    } else {
        PVec::add_xy(first, 1.0, 1.0)
    };

    if n == 1 && ((taper_start == 0.0 && taper_end == 0.0) || opts.last) {
        let start = PVec::add(
            first,
            PVec::mul(PVec::sub(first, last).uni().per(), -first_radius),
        );
        let steps = arc_steps(
            first_radius,
            FIXED_PI * 2.0,
            cap_tol,
            MIN_ROUNDED_CORNER_STEPS,
            MAX_ROUNDED_CORNER_STEPS,
        );
        let step = 1.0 / steps as f64;
        let mut dot = Vec::new();
        let mut t = step;
        while t <= 1.0 {
            dot.push(PVec::rot_with(start, first, FIXED_PI * 2.0 * t));
            t += step;
        }
        return dot;
    }

    if tracks.left.is_empty() || tracks.right.is_empty() {
        return Vec::new();
    }

    let mut start_cap = Vec::new();
    if taper_start != 0.0 || (taper_end != 0.0 && n == 1) {
    } else if opts.start.cap {
        let first_right = pt(tracks.right[0]);
        let steps = arc_steps(first_radius, FIXED_PI, cap_tol, 4, 8);
        let step = 1.0 / steps as f64;
        let mut t = step;
        while t <= 1.0 {
            start_cap.push(PVec::rot_with(first_right, first, FIXED_PI * t));
            t += step;
        }
    } else {
        let corners = PVec::new(
            tracks.left[0].x - tracks.right[0].x,
            tracks.left[0].y - tracks.right[0].y,
        );
        let a = PVec::mul(corners, 0.5);
        let b = PVec::mul(corners, 0.51);
        start_cap.extend([
            PVec::sub(first, a),
            PVec::sub(first, b),
            PVec::add(first, b),
            PVec::add(first, a),
        ]);
    }

    let last_radius = src[n - 1].radius;
    let mut last_vec = (1.0, 1.0);
    if n > 1 {
        let dx = src[n - 2].p.x - src[n - 1].p.x;
        let dy = src[n - 2].p.y - src[n - 1].p.y;
        let l = (dx * dx + dy * dy).sqrt();
        last_vec = if l == 0.0 { (dx, dy) } else { (dx / l, dy / l) };
    }
    let direction = PVec::new(-last_vec.1, last_vec.0);

    let mut end_cap = Vec::new();
    if taper_end != 0.0 || (taper_start != 0.0 && n == 1) {
        end_cap.push(last);
    } else if opts.end.cap {
        let start = PVec::add(last, PVec::mul(direction, last_radius));
        let steps = arc_steps(
            last_radius,
            FIXED_PI * 3.0,
            cap_tol,
            MIN_CAP_STEPS,
            MAX_CAP_STEPS,
        );
        let step = 1.0 / steps as f64;
        let mut t = step;
        while t < 1.0 {
            end_cap.push(PVec::rot_with(start, last, FIXED_PI * 3.0 * t));
            t += step;
        }
    } else {
        end_cap.extend([
            PVec::add(last, PVec::mul(direction, last_radius)),
            PVec::add(last, PVec::mul(direction, last_radius * 0.99)),
            PVec::sub(last, PVec::mul(direction, last_radius * 0.99)),
            PVec::sub(last, PVec::mul(direction, last_radius)),
        ]);
    }

    let mut out: Vec<PVec> = tracks.left.iter().map(|p| pt(*p)).collect();
    out.extend(end_cap);
    out.extend(tracks.right.iter().rev().map(|p| pt(*p)));
    out.extend(start_cap);
    out
}
