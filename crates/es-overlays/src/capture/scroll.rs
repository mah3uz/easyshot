use es_scroll::{AutoScrollStop, Frame, Push, Settle, SettleStep, Stitcher, frame_hash};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Wayland reports no scroll events from other clients, so manual mode watches the frames instead.
pub const POLL: Duration = Duration::from_millis(150);
pub const QUIET: Duration = Duration::from_millis(250);
// The selector's surfaces are still on screen for a frame or two after they close.
pub const UNMAP: Duration = Duration::from_millis(80);
pub const AUTO_SPEED: u32 = 3;

pub trait Source {
    /// The area as it is now, or `None` when the capture failed.
    fn grab(&mut self) -> Option<Frame>;
    /// `false` when the scroll could not be sent.
    fn scroll(&mut self, dy: f64) -> bool;
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
    /// Logical points per automatic scroll.
    pub step: f64,
    /// The largest preview image, in pixels.
    pub preview: (usize, usize),
    pub max_height: usize,
}

/// Grabs until two frames in a row are the same, or gives the last one when the area never settles.
pub fn settled(src: &mut dyn Source) -> Option<Frame> {
    let mut settle = Settle::new();
    loop {
        let frame = src.grab()?;
        match settle.observe(frame_hash(&frame)) {
            SettleStep::Settled | SettleStep::GaveUp => return Some(frame),
            SettleStep::Retry(d) => src.sleep(d),
        }
    }
}

/// Nearest-neighbour, never enlarged.
pub fn fit(frame: &Frame, max: (usize, usize)) -> Frame {
    let k = (max.0 as f64 / frame.width as f64)
        .min(max.1 as f64 / frame.height as f64)
        .min(1.0);
    let w = ((frame.width as f64 * k).round() as usize).max(1);
    let h = ((frame.height as f64 * k).round() as usize).max(1);
    let mut data = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let sy = (y * frame.height / h).min(frame.height - 1);
        let row = &frame.data[sy * frame.width * 4..(sy + 1) * frame.width * 4];
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
        if matches!(push, Push::Appended(_) | Push::Full) {
            (self.emit)(Update::Height(self.stitcher.height()));
            if let Some(image) = self.stitcher.image() {
                (self.emit)(Update::Preview(fit(image, self.params.preview)));
            }
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
        if let Some(frame) = settled(src) {
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
    src.sleep(UNMAP);
    let mut session = Session {
        stitcher: Stitcher::with_max_height(params.max_height),
        params,
        emit,
    };
    let Some(first) = settled(src) else {
        return Outcome::Failed;
    };
    if session.push(&first, false) == Push::Full {
        return session.finish();
    }
    let mut auto = false;
    let mut stop = AutoScrollStop::new();
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
            if !src.scroll(params.step) {
                auto = false;
                (session.emit)(Update::Auto(false));
                continue;
            }
            let push = settled(src).map_or(Push::NoOverlap, |f| session.push(&f, false));
            if push != Push::Full && stop.record(matches!(push, Push::Appended(_))) {
                return session.finish_with(src);
            }
            push
        } else {
            src.sleep(POLL);
            if !src.pointer_inside() {
                continue;
            }
            let Some(frame) = src.grab() else {
                continue;
            };
            let (hash, now) = (frame_hash(&frame), src.now());
            if hash != seen {
                seen = hash;
                moved = Some(now);
                session.push(&frame, false)
            } else if moved.is_some_and(|t| now.duration_since(t) >= QUIET) {
                moved = None;
                settled(src).map_or(Push::NoOverlap, |f| session.push(&f, false))
            } else {
                continue;
            }
        };
        if push == Push::Full {
            return session.finish();
        }
    }
}
