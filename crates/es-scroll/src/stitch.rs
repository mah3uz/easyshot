use crate::columns::{Columns, common_colour, fixed_columns};
use crate::shift::{Exclusions, Mask, pixel_sad, shift_masked, shift_robust};
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
    strip: usize,
    bar: bool,
    moving: Vec<bool>,
    /// `moving` placed on the canvas, for an animation that scrolls with the page.
    volatile: Vec<bool>,
    fixed: Vec<bool>,
    scrolled: Vec<bool>,
    fill: Vec<[u8; 4]>,
    first_rows: usize,
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
            strip: 0,
            bar: false,
            moving: Vec::new(),
            volatile: Vec::new(),
            fixed: Vec::new(),
            scrolled: Vec::new(),
            fill: Vec::new(),
            first_rows: 0,
        }
    }

    pub fn height(&self) -> usize {
        self.canvas.as_ref().map_or(0, |c| c.height)
    }

    /// The frame rows seen changing while the rest stood still.
    pub fn moving(&self) -> &[bool] {
        &self.moving
    }

    /// The image so far, with columns that do not scroll still showing what each frame added there.
    pub fn image(&self) -> Option<&Frame> {
        self.canvas.as_ref()
    }

    /// Columns that do not scroll keep what the first frame showed; below it they take their most common colour there, so a minimap or side panel never repeats down the page.
    pub fn finish(self) -> Option<Frame> {
        let mut canvas = self.canvas?;
        let width = canvas.width;
        let end = canvas.height.saturating_sub(self.footer);
        for x in (0..self.fixed.len()).filter(|&x| self.fixed[x]) {
            for y in self.first_rows..end {
                let i = (y * width + x) * 4;
                canvas.data[i..i + 4].copy_from_slice(&self.fill[x]);
            }
        }
        Some(without_bar(canvas, if self.bar { self.strip } else { 0 }))
    }

    pub fn push(&mut self, frame: &Frame) -> Push {
        self.push_inner(frame, false, None)
    }

    /// `expected` is how far the scroll sent should have moved the page since the last frame placed.
    pub fn push_near(&mut self, frame: &Frame, expected: usize) -> Push {
        self.push_inner(frame, false, Some(expected))
    }

    pub fn push_final(&mut self, frame: &Frame) -> Push {
        self.push_inner(frame, true, None)
    }

    fn push_inner(&mut self, frame: &Frame, last: bool, prior: Option<usize>) -> Push {
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
        let cols = columns(frame.width, right, &self.fixed);
        let guess = Exclusions {
            top: self
                .header
                .max(still_edge(prev, frame, 0..frame.height, &cols)),
            bottom: self
                .footer
                .max(still_edge(prev, frame, (0..frame.height).rev(), &cols)),
            right,
        };
        // An unscrolled frame goes on to `moved_in_place`, which learns the rows that animate.
        let Some(mut dy) = self.find_shift(prev, frame, guess, prior, &self.fixed) else {
            return self.moved_in_place(frame, hash);
        };
        let mut found_cols = Columns {
            fixed: Vec::new(),
            scrolled: Vec::new(),
        };
        if dy > 0 {
            found_cols = self.fixed_columns(prev, frame, dy, guess);
            let mut wider = self.fixed.clone();
            wider.resize(frame.width, false);
            let new = found_cols.fixed.iter().filter(|&&x| !wider[x]).count();
            found_cols.fixed.iter().for_each(|&x| wider[x] = true);
            // Columns found at a wrong shift would be wrong too, so only those found again at the shift matched without them are kept.
            if new > 0
                && let Some(again) = self
                    .find_shift(prev, frame, guess, prior, &wider)
                    .filter(|&again| again > 0 && again != dy)
            {
                dy = again;
                found_cols = self.fixed_columns(prev, frame, dy, guess);
            }
        }
        if dy == 0 {
            self.prev = Some(frame.clone());
            self.prev_hash = hash;
            return Push::Duplicate;
        }
        if !last && dy < frame.height / 10 {
            return Push::Duplicate;
        }
        if dy > MIN_LOCK_SHIFT && self.right.is_none() {
            let (margin, strip) = scrollbar_margin(prev, frame, guess, dy);
            self.right = Some(margin);
            self.strip = strip;
        }
        // Only a strip that changes on screen between frames is a scrollbar; a panel that stands still keeps what the first view showed.
        if self.strip > 0 && !self.bar {
            self.bar = strip_changes(prev, frame, self.strip);
        }
        self.keep_fixed(&found_cols);
        let Some(prev) = self.prev.as_ref() else {
            return Push::NoOverlap;
        };
        let cols = columns(frame.width, self.right.unwrap_or(0), &self.fixed);
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

    fn find_shift(
        &self,
        prev: &Frame,
        frame: &Frame,
        guess: Exclusions,
        prior: Option<usize>,
        fixed: &[bool],
    ) -> Option<usize> {
        let guesses: &[usize] = match self.right {
            Some(_) => &[],
            None => PROBE_MARGINS,
        };
        let page = self.volatile.get(self.top..).unwrap_or(&[]);
        // An animation seen in place may be fixed on screen or ride with the page; each reading is tried, since masking both can leave too little to match.
        let mut masks = vec![Mask {
            screen: &self.moving,
            page: &[],
            columns: fixed,
        }];
        if page.contains(&true) {
            masks.push(Mask {
                screen: &[],
                page,
                columns: fixed,
            });
        }
        let found = [guess.right].iter().chain(guesses).find_map(|&right| {
            let ex = Exclusions { right, ..guess };
            masks
                .iter()
                .find_map(|&mask| shift_masked(prev, frame, ex, mask, prior))
        });
        found.or_else(|| shift_robust(prev, frame, guess).filter(|&dy| dy > 0))
    }

    /// The side columns that did not move with the page, less any seen carrying page content in an earlier pair.
    fn fixed_columns(&self, prev: &Frame, frame: &Frame, dy: usize, guess: Exclusions) -> Columns {
        let mut found = fixed_columns(prev, frame, dy, &self.overlap(frame.height, dy, guess));
        let scrolled = |x: usize| self.scrolled.get(x).copied().unwrap_or(false);
        found
            .fixed
            .retain(|&x| !scrolled(x) && !found.scrolled.contains(&x));
        found
    }

    /// The rows of the new frame, clear of bars and of known animation, whose page rows the previous frame shows `dy` rows lower.
    fn overlap(&self, height: usize, dy: usize, ex: Exclusions) -> Vec<usize> {
        let flagged = |m: &[bool], y: usize| m.get(y).copied().unwrap_or(false);
        let top = self.header.max(ex.top);
        let end = height.saturating_sub(dy + self.footer.max(ex.bottom));
        (top..end)
            .filter(|&y| {
                !flagged(&self.moving, y)
                    && !flagged(&self.moving, y + dy)
                    && !flagged(&self.volatile, self.top + y + dy)
            })
            .collect()
    }

    fn mark_volatile(&mut self, rows: impl Iterator<Item = usize>) {
        for y in rows {
            if self.volatile.len() <= y {
                self.volatile.resize(y + 1, false);
            }
            self.volatile[y] = true;
        }
    }

    // The canvas keeps every column's own pixels until `finish`, so a column found to carry page content later loses none of it.
    fn keep_fixed(&mut self, found: &Columns) {
        let Some(canvas) = self.canvas.as_ref() else {
            return;
        };
        let width = canvas.width;
        self.fixed.resize(width, false);
        self.scrolled.resize(width, false);
        self.fill.resize(width, [0, 0, 0, 255]);
        for &x in &found.scrolled {
            self.scrolled[x] = true;
            self.fixed[x] = false;
        }
        for &x in &found.fixed {
            if !self.fixed[x] {
                self.fixed[x] = true;
                self.fill[x] = common_colour(canvas, self.header..self.first_rows, x);
            }
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
        for &y in &changed {
            self.moving[y] = true;
        }
        let top = self.top;
        self.mark_volatile(changed.into_iter().map(|y| top + y));
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
        self.first_rows = rows;
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

// A thumb that moves with each frame would otherwise show down the right edge at every position it had.
fn without_bar(canvas: Frame, strip: usize) -> Frame {
    if strip == 0 || strip >= canvas.width {
        return canvas;
    }
    let width = canvas.width - strip;
    let mut data = Vec::with_capacity(width * canvas.height * 4);
    for y in 0..canvas.height {
        data.extend_from_slice(&canvas.row(y)[..width * 4]);
    }
    Frame {
        width,
        height: canvas.height,
        data,
    }
}

fn columns(width: usize, right: usize, fixed: &[bool]) -> Vec<usize> {
    (0..width.saturating_sub(right))
        .step_by(4)
        .filter(|&x| !fixed.get(x).copied().unwrap_or(false))
        .collect()
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

fn strip_changes(prev: &Frame, next: &Frame, strip: usize) -> bool {
    let from = (next.width - strip) * 4;
    let rows = (0..next.height)
        .filter(|&y| {
            let sum: f64 = (from..next.width * 4)
                .step_by(4)
                .map(|i| pixel_sad(&prev.row(y)[i..], &next.row(y)[i..]))
                .sum();
            sum / strip as f64 > CHANGE_SAD
        })
        .take(2)
        .count();
    rows >= 2
}

/// The margin to leave out of matching and the width of the strip found, which is the margin without its padding.
fn scrollbar_margin(prev: &Frame, next: &Frame, ex: Exclusions, dy: usize) -> (usize, usize) {
    let rows = next.height.saturating_sub(ex.top + ex.bottom + dy);
    let (from, to) = (rows / 5, rows * 4 / 5);
    let step = (to.saturating_sub(from) / 40).max(1);
    let sampled: Vec<usize> = (from..to).step_by(step).collect();
    if sampled.is_empty() {
        return (0, 0);
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
        Some(x) if (3..=40).contains(&(next.width - x)) => (
            (next.width - x + SCROLLBAR_PAD).min(next.width - 1),
            next.width - x,
        ),
        _ => (0, 0),
    }
}
