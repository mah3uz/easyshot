use crate::shift::{Exclusions, shift};
use crate::{Frame, frame_hash};

pub const MAX_HEIGHT: usize = 30_000;
const MIN_HEADER: usize = 10;
const MAX_HEADER_FRACTION: f64 = 0.6;
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
    header: Option<usize>,
    right: Option<usize>,
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
            header: None,
            right: None,
        }
    }

    pub fn height(&self) -> usize {
        self.canvas.as_ref().map_or(0, |c| c.height)
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
        let header = self.header.unwrap_or_else(|| header_height(prev, frame));
        let guesses: &[usize] = match self.right {
            Some(_) => &[],
            None => PROBE_MARGINS,
        };
        let known = [self.right.unwrap_or(0)];
        let Some(dy) = known
            .iter()
            .chain(guesses)
            .find_map(|&right| shift(prev, frame, Exclusions { top: header, right }))
        else {
            return Push::NoOverlap;
        };
        if dy == 0 {
            self.prev = Some(frame.clone());
            self.prev_hash = hash;
            return Push::Duplicate;
        }
        if !last && dy < frame.height / 10 {
            return Push::Duplicate;
        }
        if dy > MIN_LOCK_SHIFT {
            if self.header.is_none() {
                self.header = Some(header);
            }
            if self.right.is_none() {
                self.right = Some(scrollbar_margin(prev, frame, header, dy));
            }
        }
        self.merge(frame, dy);
        self.prev = Some(frame.clone());
        self.prev_hash = hash;
        if self.height() >= self.max_height {
            Push::Full
        } else {
            Push::Appended(dy)
        }
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

    fn merge(&mut self, frame: &Frame, dy: usize) {
        let Some(canvas) = self.canvas.as_mut() else {
            return;
        };
        let header = self.header.unwrap_or(0);
        self.top += dy;
        let total = (self.top + frame.height).min(self.max_height);
        let stride = canvas.stride();
        canvas.data.resize(total * stride, 0);
        canvas.height = total;
        for y in header..frame.height {
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

fn header_height(prev: &Frame, next: &Frame) -> usize {
    let cols: Vec<usize> = (0..prev.width).step_by(4).collect();
    for y in 0..prev.height {
        let (a, b) = (prev.row(y), next.row(y));
        let mean = cols
            .iter()
            .map(|&x| pixel_sad(&a[x * 4..], &b[x * 4..]))
            .sum::<f64>()
            / cols.len() as f64;
        if mean > CHANGE_SAD {
            let limit = (prev.height as f64 * MAX_HEADER_FRACTION) as usize;
            return if (MIN_HEADER..limit).contains(&y) {
                y
            } else {
                0
            };
        }
    }
    0
}

fn scrollbar_margin(prev: &Frame, next: &Frame, header: usize, dy: usize) -> usize {
    let rows = next.height.saturating_sub(header + dy);
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
                let a = &prev.row(header + dy + r)[x * 4..];
                let b = &next.row(header + r)[x * 4..];
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
