use crate::Frame;

pub const MIN_NCC: f64 = 0.9;
const SAMPLE_COLUMNS: usize = 128;
const MIN_VARIANCE: f64 = 1e-3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Exclusions {
    pub top: usize,
    pub right: usize,
}

fn profile(frame: &Frame, ex: Exclusions) -> Vec<f64> {
    let usable = frame.width.saturating_sub(ex.right);
    let step = (usable / SAMPLE_COLUMNS).max(1);
    (ex.top.min(frame.height)..frame.height)
        .map(|y| {
            let row = frame.row(y);
            let (sum, n) = (0..usable).step_by(step).fold((0.0, 0.0), |(s, n), x| {
                let p = &row[x * 4..x * 4 + 3];
                let lum = 29.0 * f64::from(p[0]) + 150.0 * f64::from(p[1]) + 77.0 * f64::from(p[2]);
                (s + lum / 256.0, n + 1.0)
            });
            if n > 0.0 { sum / n } else { 0.0 }
        })
        .collect()
}

fn ncc(a: &[f64], b: &[f64]) -> f64 {
    let m = a.len() as f64;
    let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        sa += x;
        sb += y;
        saa += x * x;
        sbb += y * y;
        sab += x * y;
    }
    let va = saa - sa * sa / m;
    let vb = sbb - sb * sb / m;
    if va < MIN_VARIANCE * m || vb < MIN_VARIANCE * m {
        return 0.0;
    }
    (sab - sa * sb / m) / (va * vb).sqrt()
}

pub fn shift(prev: &Frame, next: &Frame, ex: Exclusions) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let mut p = profile(prev, ex);
    let mut n = profile(next, ex);
    let len = p.len();
    let min_overlap = (len / 10).max(8);
    if len < min_overlap {
        return None;
    }
    for v in [&mut p, &mut n] {
        let mean = v.iter().sum::<f64>() / len as f64;
        v.iter_mut().for_each(|x| *x -= mean);
    }
    let mut best = (MIN_NCC, None);
    for dy in 0..=len - min_overlap {
        let r = ncc(&p[dy..], &n[..len - dy]);
        if r > best.0 {
            best = (r, Some(dy));
        }
    }
    best.1
}
