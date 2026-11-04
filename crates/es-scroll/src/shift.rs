use crate::Frame;

pub const MIN_NCC: f64 = 0.9;
const SAMPLE_COLUMNS: usize = 1024;
const BANDS: usize = 32;
const MIN_VARIANCE: f64 = 1e-3;
const EDGE: f64 = 2.0;
const SAME: f64 = 1.0;
const MIN_SHARE: f64 = 0.5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Exclusions {
    pub top: usize,
    pub right: usize,
}

// Text lines differ in a glyph or two, which a whole-row mean averages away; side-by-side bands keep it.
fn profile(frame: &Frame, ex: Exclusions) -> (Vec<f64>, usize) {
    let usable = frame.width.saturating_sub(ex.right);
    let step = (usable / SAMPLE_COLUMNS).max(1);
    let samples = usable.div_ceil(step).max(1);
    let bands = BANDS.min(samples);
    let mut out = Vec::with_capacity(frame.height.saturating_sub(ex.top) * bands);
    for y in ex.top.min(frame.height)..frame.height {
        let row = frame.row(y);
        let mut sums = [(0.0, 0.0); BANDS];
        for (i, x) in (0..usable).step_by(step).enumerate() {
            let p = &row[x * 4..x * 4 + 3];
            let lum = 29.0 * f64::from(p[0]) + 150.0 * f64::from(p[1]) + 77.0 * f64::from(p[2]);
            let s = &mut sums[i * bands / samples];
            *s = (s.0 + lum / 256.0, s.1 + 1.0);
        }
        out.extend(sums[..bands].iter().map(|&(s, n)| s / f64::max(n, 1.0)));
    }
    (out, bands)
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
    shift_masked(prev, next, ex, &[])
}

/// `moving` marks the frame rows whose content changes in place, which are left out of the match.
pub fn shift_masked(prev: &Frame, next: &Frame, ex: Exclusions, moving: &[bool]) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let (mut p, bands) = profile(prev, ex);
    let (mut n, _) = profile(next, ex);
    let len = p.len() / bands;
    let min_overlap = (len / 10).max(8);
    if len < min_overlap {
        return None;
    }
    for v in [&mut p, &mut n] {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.iter_mut().for_each(|x| *x -= mean);
    }
    let still = |y: usize| !moving.get(ex.top + y).copied().unwrap_or(false);
    let mut best = (MIN_NCC, None);
    for dy in 0..=len - min_overlap {
        let rows: Vec<usize> = (0..len - dy)
            .filter(|&y| still(y) && still(y + dy))
            .collect();
        if rows.len() < min_overlap {
            continue;
        }
        let r = if rows.len() == len - dy {
            ncc(&p[dy * bands..], &n[..(len - dy) * bands])
        } else {
            let band = |v: &[f64], y: usize| v[y * bands..(y + 1) * bands].to_vec();
            let a: Vec<f64> = rows.iter().flat_map(|&y| band(&p, y + dy)).collect();
            let b: Vec<f64> = rows.iter().flat_map(|&y| band(&n, y)).collect();
            ncc(&a, &b)
        };
        if r > best.0 {
            best = (r, Some(dy));
        }
    }
    best.1
}

/// Content moving by itself breaks a whole-area correlation, so this counts the edges that reappear unchanged at each shift; a fixed animation spoils only the edges it covers.
pub fn shift_robust(prev: &Frame, next: &Frame, ex: Exclusions) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let (p, bands) = profile(prev, ex);
    let (n, _) = profile(next, ex);
    let len = p.len() / bands;
    let min_overlap = (len / 10).max(8);
    if len <= min_overlap {
        return None;
    }
    let edges = edges(&n, bands);
    let mut best = (0, 0, 0);
    for dy in 0..=len - min_overlap {
        let (same, seen) = edges_kept(&p, &n, bands, &edges, dy);
        if same > best.0 {
            best = (same, seen, dy);
        }
    }
    let (same, seen, dy) = best;
    (same >= min_overlap && same as f64 >= MIN_SHARE * seen as f64).then_some(dy)
}

/// Whether most edges stayed where they were, so content animating in place does not count as a move.
pub fn unmoved(prev: &Frame, next: &Frame) -> bool {
    if prev.width != next.width || prev.height != next.height {
        return false;
    }
    let (p, bands) = profile(prev, Exclusions::default());
    let (n, _) = profile(next, Exclusions::default());
    let edges = edges(&n, bands);
    let (same, seen) = edges_kept(&p, &n, bands, &edges, 0);
    if seen == 0 {
        return p == n;
    }
    same as f64 >= MIN_SHARE * seen as f64
}

fn edges(n: &[f64], bands: usize) -> Vec<usize> {
    (bands..n.len())
        .filter(|&i| (n[i] - n[i - bands]).abs() > EDGE)
        .collect()
}

fn edges_kept(p: &[f64], n: &[f64], bands: usize, edges: &[usize], dy: usize) -> (usize, usize) {
    let limit = (n.len() / bands).saturating_sub(dy) * bands;
    let (mut seen, mut same) = (0, 0);
    for &i in edges.iter().take_while(|&&i| i < limit) {
        seen += 1;
        let j = i + dy * bands;
        if (p[j] - n[i]).abs() <= SAME && (p[j - bands] - n[i - bands]).abs() <= SAME {
            same += 1;
        }
    }
    (same, seen)
}
