mod pacer;
mod pipeline;
mod schedule;
#[cfg(test)]
mod tests;

use crate::process::run_blocking;
use crate::{
    Error, RecordSource, RecordSpec, Recorder, RecorderEvent, RecorderProbe, RecordingHandle,
    RecordingOutput, Result,
};
use es_model::VideoFrame;
use futures::future::BoxFuture;
use gstreamer as gst;
use gstreamer::prelude::ClockExt;
use pacer::{Command, Pacer, Spans, monotonic_us};
use pipeline::{ELEMENTS, Picture, Pipeline, canvas};
use schedule::Schedule;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use pacer::PacerStats;
pub use pipeline::Encoder;

#[derive(Debug)]
pub enum SourceEvent {
    Frame(VideoFrame),
    /// The window closed.
    Stopped,
    Failed(String),
}

/// Frames of one window. Dropping the source ends its capture.
pub trait FrameSource: Send {
    /// The newest frame not yet taken, then `Stopped` or `Failed` once; `None` when nothing
    /// arrives within `timeout`.
    fn recv_timeout(&mut self, timeout: Duration) -> Option<SourceEvent>;
}

type OpenSource = dyn Fn(&RecordSpec) -> Result<Box<dyn FrameSource>> + Send + Sync;

const FIRST_FRAME_WAIT: Duration = Duration::from_secs(3);

/// Records a window from its own frames into `screen.mkv`, at a constant frame rate.
pub struct WindowRecorder {
    open: Arc<OpenSource>,
    encoders: Vec<Encoder>,
}

impl WindowRecorder {
    /// `open` starts the frame source for a spec whose source is a window.
    pub fn new(
        open: impl Fn(&RecordSpec) -> Result<Box<dyn FrameSource>> + Send + Sync + 'static,
    ) -> Self {
        WindowRecorder {
            open: Arc::new(open),
            encoders: Encoder::ORDER.to_vec(),
        }
    }

    pub fn with_encoders(mut self, encoders: &[Encoder]) -> Self {
        self.encoders = encoders.to_vec();
        self
    }

    pub fn start_window(&self, spec: &RecordSpec) -> Result<WindowRecording> {
        if !matches!(spec.source, RecordSource::Window(_)) {
            return Err(Error::Unsupported);
        }
        gst::init().map_err(|e| Error::Pipeline(e.to_string()))?;
        std::fs::create_dir_all(&spec.dir).map_err(|source| Error::Io {
            path: spec.dir.clone(),
            source,
        })?;
        let file = spec.dir.join("screen.mkv");
        let fps = spec.fps.max(1);
        let mut source = (self.open)(spec)?;
        let first = Picture::new(first_frame(&mut *source)?)?;
        let pipeline = Pipeline::open(&file, canvas(first.size()), fps, &first, &self.encoders)
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&file);
            })?;
        let clock = pipeline.clock()?;
        let schedule = Schedule::new(clock.time().nseconds(), fps);
        let first_us = monotonic_us();
        let encoder = pipeline.encoder();
        let (commands, rx) = std::sync::mpsc::channel();
        let (tx, events) = async_channel::unbounded();
        let stats = Arc::new(Mutex::new(PacerStats::default()));
        let pacer = Pacer {
            pipeline,
            source,
            commands: rx,
            events: tx.clone(),
            stats: stats.clone(),
            clock,
            schedule,
            last: first,
            first_us,
        };
        let thread = std::thread::Builder::new()
            .name("es-window-pacer".into())
            .spawn(move || pacer.run())
            .map_err(|source| Error::Io {
                path: file.clone(),
                source,
            })?;
        tracing::info!(encoder = encoder.element(), "window recording started");
        let _ = tx.try_send(RecorderEvent::Started);
        Ok(WindowRecording {
            commands,
            thread: Some(thread),
            events,
            tx,
            stats,
            encoder,
            file,
            dir: spec.dir.clone(),
            first_us,
            paused: false,
        })
    }
}

fn first_frame(source: &mut dyn FrameSource) -> Result<VideoFrame> {
    let until = Instant::now() + FIRST_FRAME_WAIT;
    let mut newest = None;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        let wait = if newest.is_some() {
            Duration::ZERO
        } else {
            left
        };
        match source.recv_timeout(wait) {
            Some(SourceEvent::Frame(f)) => newest = Some(f),
            Some(SourceEvent::Stopped) => return Err(Error::WindowClosed),
            Some(SourceEvent::Failed(m)) => return Err(Error::Capture(m)),
            None => return newest.ok_or(Error::Timeout),
        }
    }
}

impl Recorder for WindowRecorder {
    fn name(&self) -> &'static str {
        "window"
    }

    fn probe(&self) -> RecorderProbe {
        let problem = if let Err(e) = gst::init() {
            Some(format!("GStreamer: {e}"))
        } else if let Some(missing) = ELEMENTS
            .iter()
            .find(|e| gst::ElementFactory::find(e).is_none())
        {
            Some(format!("GStreamer element {missing} not found"))
        } else if !self.encoders.iter().any(|e| e.installed()) {
            Some("no H.264 encoder found".to_owned())
        } else {
            None
        };
        RecorderProbe {
            available: problem.is_none(),
            reason: problem,
        }
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        Ok(Box::new(self.start_window(&spec)?))
    }

    fn can_record_system_audio(&self, _with_mic: bool) -> bool {
        false
    }
}

pub struct WindowRecording {
    commands: Sender<Command>,
    thread: Option<JoinHandle<Result<Spans>>>,
    events: async_channel::Receiver<RecorderEvent>,
    tx: async_channel::Sender<RecorderEvent>,
    stats: Arc<Mutex<PacerStats>>,
    encoder: Encoder,
    file: PathBuf,
    dir: PathBuf,
    first_us: u64,
    paused: bool,
}

impl WindowRecording {
    pub fn encoder(&self) -> Encoder {
        self.encoder
    }

    pub fn stats(&self) -> PacerStats {
        self.stats.lock().map(|s| *s).unwrap_or_default()
    }

    fn set_paused(&mut self, want: bool) -> Result<()> {
        if self.paused == want {
            return Ok(());
        }
        let command = if want {
            Command::Pause
        } else {
            Command::Resume
        };
        self.commands
            .send(command)
            .map_err(|_| Error::Protocol("recorder is not running".into()))?;
        self.paused = want;
        let _ = self.tx.try_send(if want {
            RecorderEvent::Paused
        } else {
            RecorderEvent::Resumed
        });
        Ok(())
    }

    fn end(&mut self, command: Command) -> Result<Spans> {
        let _ = self.commands.send(command);
        self.thread
            .take()
            .ok_or_else(|| Error::Protocol("recording already ended".into()))?
            .join()
            .map_err(|_| Error::Protocol("the pacer thread panicked".into()))?
    }
}

impl RecordingHandle for WindowRecording {
    fn pause(&mut self) -> Result<()> {
        self.set_paused(true)
    }

    fn resume(&mut self) -> Result<()> {
        self.set_paused(false)
    }

    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
        run_blocking(move || {
            let segments = self.end(Command::Stop)?;
            Ok(RecordingOutput {
                file: self.file.clone(),
                first_frame_monotonic_us: self.first_us,
                segments,
            })
        })
    }

    fn discard(mut self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        run_blocking(move || {
            let _ = self.end(Command::Discard);
            std::fs::remove_dir_all(&self.dir).map_err(|source| Error::Io {
                path: self.dir.clone(),
                source,
            })
        })
    }

    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.events.clone()
    }
}
