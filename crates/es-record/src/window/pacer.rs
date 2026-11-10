use super::pipeline::{Picture, Pipeline};
use super::schedule::Schedule;
use super::{FrameSource, SourceEvent};
use crate::{Error, RecorderEvent, Result};
use es_model::VideoFrame;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Recorded spans in CLOCK_MONOTONIC microseconds.
pub(crate) type Spans = Vec<(u64, u64)>;

const PAUSED_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Command {
    Pause,
    Resume,
    Stop,
    Discard,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PacerStats {
    /// Frames written to the file, the pre-roll frame included.
    pub ticks: u64,
    pub new_frames: u64,
    pub repeats: u64,
    /// Ticks never written: the encoder's queue was full, or the pacer ran past their deadline.
    pub skipped_ticks: u64,
}

enum End {
    Stop,
    Discard,
    WindowClosed,
    SourceFailed(String),
}

pub(crate) fn monotonic_us() -> u64 {
    let t = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    (t.tv_sec as u64) * 1_000_000 + (t.tv_nsec as u64) / 1000
}

pub(crate) struct Pacer {
    pub pipeline: Pipeline,
    pub source: Box<dyn FrameSource>,
    pub commands: Receiver<Command>,
    pub events: async_channel::Sender<RecorderEvent>,
    pub stats: Arc<Mutex<PacerStats>>,
    pub clock: gst::Clock,
    pub schedule: Schedule,
    pub last: Picture,
    pub first_us: u64,
}

impl Pacer {
    fn now(&self) -> u64 {
        self.clock.time().nseconds()
    }

    fn count(&self, f: impl FnOnce(&mut PacerStats)) {
        if let Ok(mut s) = self.stats.lock() {
            f(&mut s);
        }
    }

    /// The newest frame waiting in the source, or how the source ended.
    fn newest(
        &mut self,
        mut held: Option<VideoFrame>,
    ) -> std::result::Result<Option<VideoFrame>, End> {
        loop {
            match self.source.recv_timeout(Duration::ZERO) {
                Some(SourceEvent::Frame(f)) => held = Some(f),
                Some(SourceEvent::Stopped) => return Err(End::WindowClosed),
                Some(SourceEvent::Failed(m)) => return Err(End::SourceFailed(m)),
                None => return Ok(held),
            }
        }
    }

    fn tick(&mut self, newest: Option<VideoFrame>) -> Result<()> {
        let fresh = newest.and_then(|f| {
            Picture::new(f)
                .inspect_err(|e| tracing::warn!(error = %e, "frame dropped"))
                .ok()
        });
        let repeat = fresh.is_none();
        if let Some(p) = fresh {
            self.last = p;
        }
        let n = self.schedule.next();
        let (pts, duration) = (self.schedule.pts(n), self.schedule.duration(n));
        let pushed = self.pipeline.push(&self.last, pts, duration)?;
        let late = self.schedule.advance(self.now());
        self.count(|s| {
            if pushed {
                s.ticks += 1;
            }
            if repeat {
                s.repeats += 1;
            } else {
                s.new_frames += 1;
            }
            s.skipped_ticks += late + u64::from(!pushed);
        });
        match self.pipeline.error() {
            Some(e) => Err(Error::Pipeline(e)),
            None => Ok(()),
        }
    }

    /// Runs until stopped, discarded, the window closes or something fails.
    pub fn run(mut self) -> Result<Spans> {
        self.count(|s| {
            s.ticks = 1;
            s.new_frames = 1;
        });
        let mut segments = Vec::new();
        let mut seg_start = self.first_us;
        let mut held = None;
        let end = loop {
            let paused = self.schedule.paused();
            let wait = if paused {
                PAUSED_POLL
            } else {
                Duration::from_nanos(self.schedule.deadline().saturating_sub(self.now()))
            };
            match self.commands.recv_timeout(wait) {
                Ok(Command::Pause) if !paused => {
                    self.schedule.pause(self.now());
                    segments.push((seg_start, monotonic_us()));
                    continue;
                }
                Ok(Command::Resume) if paused => {
                    self.schedule.resume(self.now());
                    seg_start = monotonic_us();
                    continue;
                }
                Ok(Command::Pause | Command::Resume) | Err(RecvTimeoutError::Timeout) => {}
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break End::Stop,
                Ok(Command::Discard) => break End::Discard,
            }
            if !paused && self.now() < self.schedule.deadline() {
                continue;
            }
            match self.newest(held.take()) {
                Ok(f) if paused => held = f,
                Ok(f) => {
                    if let Err(e) = self.tick(f) {
                        return self.fail(e);
                    }
                }
                Err(end) => break end,
            }
        };
        if !self.schedule.paused() {
            segments.push((seg_start, monotonic_us()));
        }
        let Pacer {
            pipeline,
            source,
            events,
            ..
        } = self;
        drop(source);
        match end {
            End::Discard => {
                drop(pipeline);
                Ok(segments)
            }
            End::Stop => pipeline.finish().map(|()| segments),
            End::WindowClosed => {
                let done = pipeline.finish();
                let _ = events.try_send(match &done {
                    Ok(()) => RecorderEvent::WindowClosed,
                    Err(e) => RecorderEvent::Failed(e.to_string()),
                });
                done.map(|()| segments)
            }
            End::SourceFailed(m) => {
                let done = pipeline.finish();
                let _ = events.try_send(RecorderEvent::Failed(format!("window capture: {m}")));
                done.map(|()| segments)
            }
        }
    }

    fn fail(self, e: Error) -> Result<Spans> {
        let _ = self.events.try_send(RecorderEvent::Failed(e.to_string()));
        Err(e)
    }
}
