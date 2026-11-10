use crate::shift::{Exclusions, shift_masked, shift_robust};
use crate::{Frame, frame_hash};

pub const MAX_HEIGHT: usize = 30_000;
const MIN_BAND: usize = 10;
const MAX_BAND_DIVISOR: usize = 3;
const MIN_LOCK_SHIFT: usize = 5;
const CHANGE_SAD: f64 = 8.0;
const SCROLLBAR_SCAN: usize = 50;
const SCROLLBAR_GAP: usize = 4;
const SCROLLBAR_PAD: usize = 4;
const PROBE_MARGINS: &[usize] = &[20, 44];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    Appended(usize),
    Duplicate,
    NoOverlap,
    Full,
}

pub struct Stitcher {
    max_height: usize,
    canvas: Option<Frame>,
    prev: Option<Frame>,
    prev_hash: u64,
    top: usize,
    header: usize,
    footer: usize,
    stamped: usize,
    right: Option<usize>,
    moving: Vec<bool>,
}

impl Default for Stitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Stitcher {
    pub fn new() -> Self {
        Self::with_max_height(MAX_HEIGHT)
    }

    pub fn with_max_height(max_height: usize) -> Self {
        Self {
            max_height,
            canvas: None,
            prev: None,
            prev_hash: 0,
            top: 0,
            header: 0,
            footer: 0,
            stamped: 0,
            right: None,
            moving: Vec::new(),
        }
    }

    pub fn height(&self) -> usize {
        self.canvas.as_ref().map_or(0, |c| c.height)
    }

    /// The frame rows seen changing while the rest stood still.
    pub fn moving(&self) -> &[bool] {
        &self.moving
    }

    pub fn image(&self) -> Option<&Frame> {
        self.canvas.as_ref()
    }

    pub fn finish(self) -> Option<Frame> {
        self.canvas
    }

    pub fn push(&mut self, frame: &Frame) -> Push {
        self.push_inner(frame, false)
    }

    pub fn push_final(&mut self, frame: &Frame) -> Push {
        self.push_inner(frame, true)
    }

    fn push_inner(&mut self, frame: &Frame, last: bool) -> Push {
        let (Some(prev), Some(canvas)) = (self.prev.as_ref(), self.canvas.as_ref()) else {
            return self.seed(frame);
        };
        if prev.width != frame.width || prev.height != frame.height {
            return Push::NoOverlap;
        }
        if canvas.height >= self.max_height {
            return Push::Full;
        }
        let hash = frame_hash(frame);
        if hash == self.prev_hash {
            return Push::Duplicate;
        }
        let right = self.right.unwrap_or(0);
        let cols = columns(frame.width, right);
        let guess = Exclusions {
            top: self
                .header
                .max(still_edge(prev, frame, 0..frame.height, &cols)),
            bottom: self
                .footer
                .max(still_edge(prev, frame, (0..frame.height).rev(), &cols)),
            right,
        };
        let guesses: &[usize] = match self.right {
            Some(_) => &[],
            None => PROBE_MARGINS,
        };
        let found = [right].iter().chain(guesses).find_map(|&right| {
            shift_masked(prev, frame, Exclusions { right, ..guess }, &self.moving)
        });
        // An unscrolled frame goes on to `moved_in_place`, which learns the rows that animate.
        let robust = || shift_robust(prev, frame, guess).filter(|&dy| dy > 0);
        let Some(dy) = found.or_else(robust) else {
            return self.moved_in_place(frame, hash);
        };
        if dy == 0 {
            self.prev = Some(frame.clone());
            self.prev_hash = hash;
            return Push::Duplicate;
        }
        if !last && dy < frame.height / 10 {
            return Push::Duplicate;
        }
        if dy > MIN_LOCK_SHIFT && self.right.is_none() {
            self.right = Some(scrollbar_margin(prev, frame, guess, dy));
        }
        let cols = columns(frame.width, self.right.unwrap_or(0));
        let below = |y: usize| Some(y + dy).filter(|&z| z < frame.height);
        let (header, header_still) = fixed_edge(frame, prev, 0..frame.height, below, &cols);
        let above = |y: usize| y.checked_sub(dy);
        let (footer, footer_still) = fixed_edge(prev, frame, (0..frame.height).rev(), above, &cols);
        // A band that changes on screen may be an animation rather than a bar, and leaving it out of every later match could leave too little to match.
        if footer_still && footer >= MIN_BAND {
            self.footer = self.footer.max(footer);
        }
        if header_still && header >= MIN_BAND {
            self.header = self.header.max(header);
            // The page's top shows the bar once, even one that appeared only after the first frame.
            if header > self.stamped {
                self.stamp(frame, header);
            }
        }
        self.merge(frame, dy, footer);
        self.prev = Some(frame.clone());
        self.prev_hash = hash;
        if self.height() >= self.max_height {
            Push::Full
        } else {
            Push::Appended(dy)
        }
    }

    /// A frame that differs from the last in under half its rows was not scrolled: those rows move by themselves.
    fn moved_in_place(&mut self, frame: &Frame, hash: u64) -> Push {
        let Some(prev) = self.prev.as_ref() else {
            return Push::NoOverlap;
        };
        let changed: Vec<usize> = (0..frame.height)
            .filter(|&y| prev.row(y) != frame.row(y))
            .collect();
        if changed.len() * 2 >= frame.height {
            return Push::NoOverlap;
        }
        self.moving.resize(frame.height, false);
        for y in changed {
            self.moving[y] = true;
        }
        self.prev = Some(frame.clone());
        self.prev_hash = hash;
        Push::Duplicate
    }

    fn seed(&mut self, frame: &Frame) -> Push {
        let rows = frame.height.min(self.max_height);
        let mut canvas = frame.clone();
        canvas.data.truncate(rows * frame.stride());
        canvas.height = rows;
        self.canvas = Some(canvas);
        self.prev = Some(frame.clone());
        self.prev_hash = frame_hash(frame);
        if rows >= self.max_height {
            Push::Full
        } else {
            Push::Appended(rows)
        }
    }

    fn stamp(&mut self, frame: &Frame, rows: usize) {
        if let Some(canvas) = self.canvas.as_mut() {
            let n = rows.min(canvas.height) * canvas.stride();
            canvas.data[..n].copy_from_slice(&frame.data[..n]);
        }
        self.stamped = rows;
    }

    /// Rows already placed are kept; the canvas ends with the previous frame's `footer` rows, which the new frame's own bottom replaces.
    fn merge(&mut self, frame: &Frame, dy: usize, footer: usize) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let placed = canvas.height.saturating_sub(footer);
        self.top += dy;
        let total = (self.top + frame.height).min(self.max_height);
        let stride = canvas.stride();
        canvas.data.resize(total * stride, 0);
        canvas.height = total;
        let from = placed.saturating_sub(self.top).max(self.header);
        for y in from..frame.height {
            let cy = self.top + y;
            if cy >= total {
                break;
            }
            canvas.data[cy * stride..(cy + 1) * stride].copy_from_slice(frame.row(y));
        }
    }
}

fn pixel_sad(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .take(3)
        .map(|(&x, &y)| f64::from(x.abs_diff(y)))
        .sum()
}

fn columns(width: usize, right: usize) -> Vec<usize> {
    (0..width.saturating_sub(right)).step_by(4).collect()
}

fn same(a: &[u8], b: &[u8], cols: &[usize]) -> bool {
    let sum: f64 = cols
        .iter()
        .map(|&x| pixel_sad(&a[x * 4..], &b[x * 4..]))
        .sum();
    sum <= CHANGE_SAD * cols.len() as f64
}

/// Before the shift is known, the rows from one edge that stand still on screen are the best guess at a fixed bar.
fn still_edge(
    prev: &Frame,
    next: &Frame,
    rows: impl Iterator<Item = usize>,
    cols: &[usize],
) -> usize {
    let cap = prev.height / MAX_BAND_DIVISOR;
    let run = rows
        .take(cap)
        .take_while(|&y| same(prev.row(y), next.row(y), cols))
        .count();
    if (MIN_BAND..cap).contains(&run) {
        run
    } else {
        0
    }
}

/// The rows from one edge of `a` that did not move with the page, and whether they all stood still on screen; `scrolled` maps a row of `a` to the row of `b` that shows the same page row.
fn fixed_edge(
    a: &Frame,
    b: &Frame,
    rows: impl Iterator<Item = usize>,
    scrolled: impl Fn(usize) -> Option<usize>,
    cols: &[usize],
) -> (usize, bool) {
    let (mut len, mut still) = (0, true);
    for (i, y) in rows.take(a.height / MAX_BAND_DIVISOR).enumerate() {
        let Some(z) = scrolled(y) else {
            break;
        };
        let here = same(a.row(y), b.row(y), cols);
        if !same(a.row(y), b.row(z), cols) {
            len = i + 1;
            still &= here;
        } else if !here {
            break;
        }
    }
    (len, still)
}

fn scrollbar_margin(prev: &Frame, next: &Frame, ex: Exclusions, dy: usize) -> usize {
    let rows = next.height.saturating_sub(ex.top + ex.bottom + dy);
    let (from, to) = (rows / 5, rows * 4 / 5);
    let step = (to.saturating_sub(from) / 40).max(1);
    let sampled: Vec<usize> = (from..to).step_by(step).collect();
    if sampled.is_empty() {
        return 0;
    }
    let changing = |x: usize| {
        let sum: f64 = sampled
            .iter()
            .map(|&r| {
                let a = &prev.row(ex.top + dy + r)[x * 4..];
                let b = &next.row(ex.top + r)[x * 4..];
                pixel_sad(a, b)
            })
            .sum();
        sum / sampled.len() as f64 > CHANGE_SAD
    };
    let mut left = None;
    let mut gap = 0;
    for x in (next.width.saturating_sub(SCROLLBAR_SCAN)..next.width).rev() {
        if changing(x) {
            left = Some(x);
            gap = 0;
        } else {
            gap += 1;
            if left.is_some() && gap > SCROLLBAR_GAP {
                break;
            }
        }
    }
    match left {
        Some(x) if (3..=40).contains(&(next.width - x)) => {
            (next.width - x + SCROLLBAR_PAD).min(next.width - 1)
        }
        _ => 0,
    }
}
