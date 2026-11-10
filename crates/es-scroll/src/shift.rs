use crate::Frame;

pub const MIN_NCC: f64 = 0.9;
const SAMPLE_COLUMNS: usize = 1024;
const BANDS: usize = 32;
const MIN_VARIANCE: f64 = 1e-3;
const EDGE: f64 = 2.0;
const SAME: f64 = 1.0;
const MIN_SHARE: f64 = 0.5;
const MAX_CANDIDATES: usize = 8;
const EXACT_COLUMNS: usize = 512;
const EXACT_TIE: f64 = 0.005;
const COLUMN_MATCH: f64 = 0.9;
pub(crate) const PIXEL_SAME: f64 = 24.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Exclusions {
    pub top: usize,
    pub bottom: usize,
    pub right: usize,
}

/// What to leave out of a match besides the edge bands.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mask<'a> {
    /// Screen rows whose content changes in place.
    pub screen: &'a [bool],
    /// Rows of `prev` showing page content that changes by itself, wherever it has scrolled to.
    pub page: &'a [bool],
    /// Columns that do not scroll with the page.
    pub columns: &'a [bool],
}

// Text lines differ in a glyph or two, which a whole-row mean averages away; side-by-side bands keep it.
fn profile(frame: &Frame, ex: Exclusions, skip: &[bool]) -> (Vec<f64>, usize) {
    let usable = kept_columns(frame.width, ex.right, skip);
    let step = (usable.len() / SAMPLE_COLUMNS).max(1);
    let samples = usable.len().div_ceil(step).max(1);
    let bands = BANDS.min(samples);
    let end = frame.height.saturating_sub(ex.bottom);
    let mut out = Vec::with_capacity(end.saturating_sub(ex.top) * bands);
    for y in ex.top.min(end)..end {
        let row = frame.row(y);
        let mut sums = [(0.0, 0.0); BANDS];
        for (i, &x) in usable.iter().step_by(step).enumerate() {
            let p = &row[x * 4..x * 4 + 3];
            let lum = 29.0 * f64::from(p[0]) + 150.0 * f64::from(p[1]) + 77.0 * f64::from(p[2]);
            let s = &mut sums[i * bands / samples];
            *s = (s.0 + lum / 256.0, s.1 + 1.0);
        }
        out.extend(sums[..bands].iter().map(|&(s, n)| s / f64::max(n, 1.0)));
    }
    (out, bands)
}

fn kept_columns(width: usize, right: usize, skip: &[bool]) -> Vec<usize> {
    (0..width.saturating_sub(right))
        .filter(|&x| !skip.get(x).copied().unwrap_or(false))
        .collect()
}

pub(crate) fn pixel_sad(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .take(3)
        .map(|(&x, &y)| f64::from(x.abs_diff(y)))
        .sum()
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
    shift_masked(prev, next, ex, Mask::default(), None)
}

/// Band correlation finds the candidate shifts, a pixel comparison picks among them, and `prior`, the shift the scroll sent should give, settles a tie such as repeating text lines make.
pub fn shift_masked(
    prev: &Frame,
    next: &Frame,
    ex: Exclusions,
    mask: Mask,
    prior: Option<usize>,
) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let (mut p, bands) = profile(prev, ex, mask.columns);
    let (mut n, _) = profile(next, ex, mask.columns);
    let len = p.len() / bands;
    let min_overlap = (len / 10).max(8);
    if len < min_overlap {
        return None;
    }
    for v in [&mut p, &mut n] {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.iter_mut().for_each(|x| *x -= mean);
    }
    let flagged = |m: &[bool], y: usize| m.get(ex.top + y).copied().unwrap_or(false);
    let rows_at = |dy: usize| -> Vec<usize> {
        (0..len - dy)
            .filter(|&y| {
                !flagged(mask.screen, y)
                    && !flagged(mask.screen, y + dy)
                    && !flagged(mask.page, y + dy)
            })
            .collect()
    };
    let mut scores = vec![f64::NEG_INFINITY; len - min_overlap + 1];
    for (dy, score) in scores.iter_mut().enumerate() {
        let rows = rows_at(dy);
        if rows.len() < min_overlap {
            continue;
        }
        *score = if rows.len() == len - dy {
            ncc(&p[dy * bands..], &n[..(len - dy) * bands])
        } else {
            let band = |v: &[f64], y: usize| v[y * bands..(y + 1) * bands].to_vec();
            let a: Vec<f64> = rows.iter().flat_map(|&y| band(&p, y + dy)).collect();
            let b: Vec<f64> = rows.iter().flat_map(|&y| band(&n, y)).collect();
            ncc(&a, &b)
        };
    }
    let peak = |dy: usize| {
        let r = scores[dy];
        r > MIN_NCC
            && dy.checked_sub(1).is_none_or(|d| scores[d] <= r)
            && scores.get(dy + 1).is_none_or(|&s| s <= r)
    };
    let mut candidates: Vec<usize> = (0..scores.len()).filter(|&dy| peak(dy)).collect();
    candidates.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    candidates.truncate(MAX_CANDIDATES);
    if candidates.len() < 2 {
        return candidates.first().copied();
    }
    let cols = kept_columns(prev.width, ex.right, mask.columns);
    let step = (cols.len() / EXACT_COLUMNS).max(1);
    let cols: Vec<usize> = cols.into_iter().step_by(step).collect();
    let shares: Vec<Vec<f64>> = candidates
        .iter()
        .map(|&dy| agreement(prev, next, ex.top, dy, &rows_at(dy), &cols))
        .collect();
    // A column that matches at no candidate, such as a minimap not yet found, says nothing about which one is right.
    let telling: Vec<usize> = (0..cols.len())
        .filter(|&i| shares.iter().any(|s| s[i] >= COLUMN_MATCH))
        .collect();
    let exact: Vec<f64> = shares
        .iter()
        .map(|s| telling.iter().map(|&i| s[i]).sum::<f64>() / telling.len().max(1) as f64)
        .collect();
    let best = exact.iter().copied().fold(0.0, f64::max);
    candidates
        .iter()
        .zip(&exact)
        .filter(|&(_, &e)| e >= best - EXACT_TIE)
        .map(|(&dy, _)| dy)
        .min_by_key(|&dy| prior.map_or(0, |p| dy.abs_diff(p)))
}

/// Per column, the share of compared rows whose pixel is the same in both frames once `next` is placed `dy` rows down.
fn agreement(
    prev: &Frame,
    next: &Frame,
    top: usize,
    dy: usize,
    rows: &[usize],
    cols: &[usize],
) -> Vec<f64> {
    let mut same = vec![0usize; cols.len()];
    for &y in rows {
        let (a, b) = (prev.row(top + y + dy), next.row(top + y));
        for (n, &x) in same.iter_mut().zip(cols) {
            *n += usize::from(pixel_sad(&a[x * 4..], &b[x * 4..]) <= PIXEL_SAME);
        }
    }
    let rows = rows.len().max(1) as f64;
    same.into_iter().map(|n| n as f64 / rows).collect()
}

/// Content moving by itself breaks a whole-area correlation, so this counts the edges that reappear unchanged at each shift; a fixed animation spoils only the edges it covers.
pub fn shift_robust(prev: &Frame, next: &Frame, ex: Exclusions) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let (p, bands) = profile(prev, ex, &[]);
    let (n, _) = profile(next, ex, &[]);
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
    let (p, bands) = profile(prev, Exclusions::default(), &[]);
    let (n, _) = profile(next, Exclusions::default(), &[]);
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
