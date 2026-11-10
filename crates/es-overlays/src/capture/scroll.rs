use es_scroll::{
    AutoScrollStop, Frame, Push, Settle, SettleStep, Stitcher, auto_scroll_step, frame_hash,
    still_hash, unmoved,
};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Wayland reports no scroll events from other clients, so manual mode watches the frames, often enough that a quick spin of the wheel still leaves consecutive frames overlapping.
pub const POLL: Duration = Duration::from_millis(50);
pub const QUIET: Duration = Duration::from_millis(250);
pub const AUTO_SPEED: u32 = 3;

pub trait Source {
    /// The area as it is now, or `None` when the capture failed.
    fn grab(&mut self) -> Option<Frame>;
    /// Wheel clicks down, or up when negative; `false` when the scroll could not be sent.
    fn scroll(&mut self, clicks: i32) -> bool;
    fn pointer_inside(&self) -> bool;
    fn sleep(&mut self, d: Duration);
    fn now(&self) -> Instant;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Stop,
    Cancel,
    Auto(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    /// The stitched height in frame pixels.
    Height(usize),
    Preview(Frame),
    Auto(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Done(Frame),
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    pub auto_available: bool,
    /// The largest preview image, in pixels.
    pub preview: (usize, usize),
    pub max_height: usize,
}

/// Grabs until a frame matches the one before outside what moves in place, or gives the last one when the area never settles.
pub fn settled(src: &mut dyn Source, moving: &[bool]) -> Option<Frame> {
    let mut settle = Settle::new();
    let mut last: Option<Frame> = None;
    loop {
        let frame = src.grab()?;
        let unchanged = last.as_ref().is_some_and(|l| {
            still_hash(l, moving) == still_hash(&frame, moving) || unmoved(l, &frame)
        });
        match settle.observe(unchanged) {
            SettleStep::Settled | SettleStep::GaveUp => return Some(frame),
            SettleStep::Retry(d) => {
                last = Some(frame);
                src.sleep(d);
            }
        }
    }
}

/// Nearest-neighbour, never enlarged.
pub fn fit(frame: &Frame, max: (usize, usize)) -> Frame {
    fit_live(frame, None, max)
}

/// The stitched image with its last view replaced by `latest`, so moving content and an unmatched frame show as they are now.
pub fn fit_live(frame: &Frame, latest: Option<&Frame>, max: (usize, usize)) -> Frame {
    let latest = latest.filter(|l| l.width == frame.width && l.height <= frame.height);
    let live_from = latest.map_or(frame.height, |l| frame.height - l.height);
    let k = (max.0 as f64 / frame.width as f64)
        .min(max.1 as f64 / frame.height as f64)
        .min(1.0);
    let w = ((frame.width as f64 * k).round() as usize).max(1);
    let h = ((frame.height as f64 * k).round() as usize).max(1);
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let sy = (y * frame.height / h).min(frame.height - 1);
        let row = match latest {
            Some(l) if sy >= live_from => {
                let y = sy - live_from;
                &l.data[y * l.width * 4..(y + 1) * l.width * 4]
            }
            _ => &frame.data[sy * frame.width * 4..(sy + 1) * frame.width * 4],
        };
        for x in 0..w {
            let sx = (x * frame.width / w).min(frame.width - 1);
            data.extend_from_slice(&row[sx * 4..sx * 4 + 4]);
        }
    }
    Frame {
        width: w,
        height: h,
        data,
    }
}

/// Each app picks how far one wheel click moves it, so the clicks per step are learned from the shifts the stitcher measures.
struct Pace {
    target: usize,
    per_click: Option<f64>,
    /// Clicks sent, less those sent back, since the stitcher last placed a frame.
    unmatched: i32,
    /// The fewest of those at which a frame found no overlap.
    ceiling: Option<i32>,
    failed: bool,
}

impl Pace {
    fn new(frame_height: usize) -> Self {
        Pace {
            target: auto_scroll_step(frame_height as u32, AUTO_SPEED) as usize,
            per_click: None,
            unmatched: 0,
            ceiling: None,
            failed: false,
        }
    }

    /// Clicks to send now, back up when negative. Until a shift is measured each step doubles the clicks so far, so no step can jump past the area; a frame that found no overlap sends the page back to half as far, and it never goes as far again.
    fn clicks(&self) -> i32 {
        let grown = match self.per_click {
            Some(p) => self.unmatched + ((self.target as f64 / p).floor() as i32).max(1),
            None => (self.unmatched * 2).max(1),
        };
        let net = match self.ceiling {
            Some(c) if self.failed => c / 2,
            Some(c) => grown.min(c - 1),
            None => grown,
        };
        net.max(1) - self.unmatched
    }

    fn sent(&mut self, clicks: i32) {
        self.unmatched += clicks;
    }

    /// How far the clicks sent should have moved the page since the last frame placed.
    fn expected(&self) -> Option<usize> {
        let p = self.per_click?;
        (self.unmatched > 0).then(|| (f64::from(self.unmatched) * p).round() as usize)
    }

    fn observe(&mut self, push: Push) {
        self.failed = push == Push::NoOverlap;
        match push {
            Push::Appended(dy) => {
                if self.unmatched > 0 {
                    self.per_click = Some(dy as f64 / f64::from(self.unmatched));
                }
                self.unmatched = 0;
                self.ceiling = None;
            }
            Push::NoOverlap => {
                let at = self.unmatched;
                self.ceiling = Some(self.ceiling.map_or(at, |c| c.min(at)));
            }
            Push::Duplicate | Push::Full => {}
        }
    }
}

struct Session<'a> {
    stitcher: Stitcher,
    params: Params,
    emit: &'a mut dyn FnMut(Update),
}

impl Session<'_> {
    fn push(&mut self, frame: &Frame, last: bool) -> Push {
        let push = if last {
            self.stitcher.push_final(frame)
        } else {
            self.stitcher.push(frame)
        };
        self.show(frame, push)
    }

    fn push_near(&mut self, frame: &Frame, expected: Option<usize>) -> Push {
        let push = match expected {
            Some(dy) => self.stitcher.push_near(frame, dy),
            None => self.stitcher.push(frame),
        };
        self.show(frame, push)
    }

    fn show(&mut self, frame: &Frame, push: Push) -> Push {
        if matches!(push, Push::Appended(_) | Push::Full) {
            (self.emit)(Update::Height(self.stitcher.height()));
        }
        if let Some(image) = self.stitcher.image() {
            let latest = (push != Push::Full).then_some(frame);
            (self.emit)(Update::Preview(fit_live(
                image,
                latest,
                self.params.preview,
            )));
        }
        push
    }

    fn finish(self) -> Outcome {
        self.stitcher
            .finish()
            .map_or(Outcome::Failed, Outcome::Done)
    }

    /// One last settled frame, which may add less than the usual minimum.
    fn finish_with(mut self, src: &mut dyn Source) -> Outcome {
        if let Some(frame) = settled(src, self.stitcher.moving()) {
            self.push(&frame, true);
        }
        self.finish()
    }
}

/// Runs one capture until it is stopped, cancelled, full, or auto-scroll runs out of page.
pub fn run(
    src: &mut dyn Source,
    commands: &Receiver<Command>,
    params: Params,
    emit: &mut dyn FnMut(Update),
) -> Outcome {
    let mut session = Session {
        stitcher: Stitcher::with_max_height(params.max_height),
        params,
        emit,
    };
    let Some(first) = settled(src, &[]) else {
        return Outcome::Failed;
    };
    if session.push(&first, false) == Push::Full {
        return session.finish();
    }
    // Auto-scroll only pushes scrolled frames, so an unscrolled one has to show which rows animate by themselves.
    if let Some(still) = src.grab() {
        session.push(&still, false);
    }
    let mut auto = false;
    let mut stop = AutoScrollStop::new();
    let mut pace = Pace::new(first.height);
    let mut seen = frame_hash(&first);
    let mut moved: Option<Instant> = None;
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Stop) => return session.finish_with(src),
                Ok(Command::Cancel) | Err(TryRecvError::Disconnected) => {
                    return Outcome::Cancelled;
                }
                Ok(Command::Auto(on)) => {
                    auto = on && params.auto_available;
                    stop = AutoScrollStop::new();
                    (session.emit)(Update::Auto(auto));
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        let push = if auto {
            let clicks = pace.clicks();
            if clicks != 0 && !src.scroll(clicks) {
                auto = false;
                (session.emit)(Update::Auto(false));
                continue;
            }
            pace.sent(clicks);
            let push = settled(src, session.stitcher.moving())
                .map_or(Push::NoOverlap, |f| session.push_near(&f, pace.expected()));
            pace.observe(push);
            if push != Push::Full && stop.record(matches!(push, Push::Appended(_))) {
                return session.finish_with(src);
            }
            push
        } else {
            src.sleep(POLL);
            if !src.pointer_inside() {
                tracing::debug!(inside = false, "scroll capture poll");
                continue;
            }
            let Some(frame) = src.grab() else {
                tracing::debug!(inside = true, grabbed = false, "scroll capture poll");
                continue;
            };
            let (hash, now) = (frame_hash(&frame), src.now());
            let changed = hash != seen;
            let push = if changed {
                seen = hash;
                moved = Some(now);
                session.push(&frame, false)
            } else if moved.is_some_and(|t| now.duration_since(t) >= QUIET) {
                moved = None;
                settled(src, session.stitcher.moving())
                    .map_or(Push::NoOverlap, |f| session.push(&f, false))
            } else {
                tracing::debug!(inside = true, changed, "scroll capture poll");
                continue;
            };
            tracing::debug!(
                inside = true,
                changed,
                ?push,
                height = session.stitcher.height(),
                "scroll capture poll"
            );
            push
        };
        if push == Push::Full {
            return session.finish();
        }
    }
}
