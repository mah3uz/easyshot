use crate::Frame;
use crate::shift::{PIXEL_SAME, pixel_sad};

const MAX_BAND_DIVISOR: usize = 3;
const MIN_DIFF_ROWS: usize = 2;
const STAY_SHARE: f64 = 0.25;
const BASELINE_STEP: usize = 4;
const MIN_SCROLLED_EDGES: usize = 2;
const SCROLLED_SHARE: f64 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Moves,
    Stays,
    Blank,
}

/// Edges of one column: rows where either frame changes colour from the row above, and those of them both frames show at the shift.
struct Counts {
    edges: usize,
    kept: usize,
    /// Edges of the new frame that only the shift explains, not standing still.
    carried: usize,
    /// Edges of the new frame that neither the shift nor standing still explains.
    unexplained: usize,
}

impl Counts {
    fn lost(&self) -> usize {
        self.edges - self.kept
    }
}

pub(crate) struct Columns {
    /// Side columns that did not move with the page.
    pub fixed: Vec<usize>,
    /// Columns whose edges moved with the page while nothing else there moved its own way, so they show page content whatever they show at other times.
    pub scrolled: Vec<usize>,
}

fn pixel(f: &Frame, y: usize, x: usize) -> &[u8] {
    &f.row(y)[x * 4..x * 4 + 4]
}

// Only edges count: a glow fixed behind the page differs at every shift without being a panel, and the content crossing it keeps its edges.
fn counts(prev: &Frame, next: &Frame, dy: usize, rows: &[usize], x: usize) -> Counts {
    let same = |a: &[u8], b: &[u8]| pixel_sad(a, b) <= PIXEL_SAME;
    let edge = |a: &[u8], b: &[u8]| !same(a, b);
    let mut c = Counts {
        edges: 0,
        kept: 0,
        carried: 0,
        unexplained: 0,
    };
    for &y in rows.iter().filter(|&&y| y > 0) {
        let (n0, n1) = (pixel(next, y - 1, x), pixel(next, y, x));
        let (p0, p1) = (pixel(prev, y + dy - 1, x), pixel(prev, y + dy, x));
        let (s0, s1) = (pixel(prev, y - 1, x), pixel(prev, y, x));
        let in_next = edge(n0, n1);
        let shifted = in_next && edge(p0, p1) && (same(n0, p0) || same(n1, p1));
        let still = in_next && edge(s0, s1) && (same(n0, s0) || same(n1, s1));
        c.edges += usize::from(in_next || edge(p0, p1));
        c.kept += usize::from(shifted);
        c.carried += usize::from(shifted && !still);
        c.unexplained += usize::from(in_next && !shifted && !still);
    }
    c
}

/// The columns at either side that did not move with the page, such as a minimap, a side panel or a scrollbar of any width; `rows` are the rows of `next` whose page rows `prev` also shows, `dy` rows lower.
pub(crate) fn fixed_columns(prev: &Frame, next: &Frame, dy: usize, rows: &[usize]) -> Columns {
    let width = next.width;
    let cap = width / MAX_BAND_DIVISOR;
    if rows.is_empty() || cap == 0 {
        return Columns {
            fixed: Vec::new(),
            scrolled: Vec::new(),
        };
    }
    // Content the compositor resamples never lines up exactly, so a side column has to disagree well beyond what the middle of the page does.
    let (lost, edges) = (cap..width - cap)
        .step_by(BASELINE_STEP)
        .map(|x| counts(prev, next, dy, rows, x))
        .fold((0, 0), |(l, e), c| (l + c.lost(), e + c.edges));
    let threshold = STAY_SHARE.max(2.0 * lost as f64 / edges.max(1) as f64);
    let side: Vec<usize> = (0..cap).chain(width - cap..width).collect();
    let side_counts: Vec<Counts> = side
        .iter()
        .map(|&x| counts(prev, next, dy, rows, x))
        .collect();
    let at = |x: usize| {
        let i = if x < cap { x } else { x - (width - cap) + cap };
        &side_counts[i]
    };
    let kind = |x: usize| {
        let c = at(x);
        if c.lost() >= MIN_DIFF_ROWS && c.lost() as f64 > threshold * c.edges as f64 {
            Kind::Stays
        } else if c.edges > 0 {
            Kind::Moves
        } else {
            Kind::Blank
        }
    };
    let band = |side: &mut dyn Iterator<Item = usize>| -> Vec<usize> {
        let mut seen = Vec::new();
        let mut kept = 0;
        for x in side {
            match kind(x) {
                Kind::Moves => break,
                Kind::Stays => {
                    seen.push(x);
                    kept = seen.len();
                }
                Kind::Blank => seen.push(x),
            }
        }
        seen.truncate(kept);
        seen
    };
    let mut fixed = band(&mut (0..cap));
    fixed.extend(band(&mut (width - cap..width).rev()));
    let scrolled = side
        .into_iter()
        .filter(|&x| {
            let c = at(x);
            c.carried >= MIN_SCROLLED_EDGES
                && c.carried as f64 >= SCROLLED_SHARE * (c.carried + c.unexplained) as f64
        })
        .collect();
    Columns { fixed, scrolled }
}

/// The colour a column shows most within `rows` of `frame`.
pub(crate) fn common_colour(frame: &Frame, rows: std::ops::Range<usize>, x: usize) -> [u8; 4] {
    let mut px: Vec<[u8; 4]> = rows
        .filter(|&y| y < frame.height)
        .map(|y| {
            let p = &frame.row(y)[x * 4..x * 4 + 4];
            [p[0], p[1], p[2], p[3]]
        })
        .collect();
    px.sort_unstable();
    px.chunk_by(|a, b| a == b)
        .max_by_key(|run| run.len())
        .map_or([0, 0, 0, 255], |run| run[0])
}
