use crate::Frame;
use crate::shift::{PIXEL_SAME, pixel_sad};

const MAX_BAND_DIVISOR: usize = 3;
const MIN_DIFF_ROWS: usize = 2;
const STAY_SHARE: f64 = 0.25;
const BASELINE_STEP: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Moves,
    Stays,
    Blank,
}

struct Counts {
    diff: usize,
    edges: usize,
}

fn pixel(f: &Frame, y: usize, x: usize) -> &[u8] {
    &f.row(y)[x * 4..x * 4 + 4]
}

fn counts(prev: &Frame, next: &Frame, dy: usize, rows: &[usize], x: usize) -> Counts {
    let differ = |a: &[u8], b: &[u8]| pixel_sad(a, b) > PIXEL_SAME;
    let (mut diff, mut edges) = (0, 0);
    for &y in rows {
        let (a, b) = (pixel(prev, y + dy, x), pixel(next, y, x));
        diff += usize::from(differ(a, b));
        let edge =
            y > 0 && (differ(b, pixel(next, y - 1, x)) || differ(a, pixel(prev, y + dy - 1, x)));
        edges += usize::from(edge);
    }
    Counts { diff, edges }
}

/// The columns at either side that did not move with the page, such as a minimap, a side panel or a scrollbar of any width; `rows` are the rows of `next` whose page rows `prev` also shows, `dy` rows lower.
pub(crate) fn fixed_columns(prev: &Frame, next: &Frame, dy: usize, rows: &[usize]) -> Vec<usize> {
    let width = next.width;
    let cap = width / MAX_BAND_DIVISOR;
    if rows.is_empty() || cap == 0 {
        return Vec::new();
    }
    // Content the compositor resamples never lines up exactly, so a side column has to disagree well beyond what the middle of the page does.
    let (diff, edges) = (cap..width - cap)
        .step_by(BASELINE_STEP)
        .map(|x| counts(prev, next, dy, rows, x))
        .fold((0, 0), |(d, e), c| (d + c.diff, e + c.edges));
    let threshold = STAY_SHARE.max(2.0 * diff as f64 / edges.max(1) as f64);
    let kind = |x: usize| {
        let c = counts(prev, next, dy, rows, x);
        if c.diff >= MIN_DIFF_ROWS && c.diff as f64 > threshold * c.edges as f64 {
            Kind::Stays
        } else if c.edges > 0 || c.diff > 0 {
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
    let mut out = band(&mut (0..cap));
    out.extend(band(&mut (width - cap..width).rev()));
    out
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
