use es_geom::Rect;
use es_model::platform::{OutputId, WindowInfo};
use futures::future::BoxFuture;
use std::path::PathBuf;

pub mod audio;
pub mod clip;
pub mod gsr;
pub mod input;
pub mod mute;
pub mod process;
pub mod project;
pub mod recovery;
pub mod units;
pub mod wf;
pub mod window;

pub use audio::{AudioDevice, AudioKind, list_audio_devices, parse_pactl_sources};
pub use gsr::GsrRecorder;
pub use input::{InputBuilder, InputCollector, RawEvent, movie_time, write_input};
pub use mute::AudioSource;
pub use process::{Clock, SystemClock, Timing};
pub use project::{CommandRunner, ProjectFolder, SystemRunner, make_poster};
pub use recovery::{RecoveryReport, recover};
pub use units::{RegionUnits, decide_units, parse_monitor_list};
pub use wf::WfRecorder;
pub use window::{Encoder, FrameSource, PacerStats, SourceEvent, WindowRecorder, WindowRecording};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unsupported on this system")]
    Unsupported,
    #[error("cancelled")]
    Cancelled,
    #[error("timed out")]
    Timeout,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("an audio device was not found")]
    AudioDeviceMissing,
    #[error("the capture area is not on any monitor")]
    AreaOffMonitor,
    #[error("the window closed")]
    WindowClosed,
    #[error("window capture failed: {0}")]
    Capture(String),
    #[error("video pipeline: {0}")]
    Pipeline(String),
    #[error("sound recording: {0}")]
    Audio(String),
    #[error("no video encoder works: {0}")]
    NoEncoder(String),
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Recorder: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self) -> RecorderProbe;
    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>>;

    /// Called before `start` with the current outputs, for recorders whose region units depend on them.
    fn prepare(&self, _spec: &RecordSpec, _outputs: &[es_model::platform::OutputInfo]) {}

    /// False when this recorder cannot add system sound to a recording, so the caller records without it.
    fn can_record_system_audio(&self, _with_mic: bool) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderProbe {
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RecordSource {
    Output(OutputId),
    /// `logical` is in global logical points, not relative to `output`.
    Region {
        output: OutputId,
        logical: Rect,
    },
    Window(WindowInfo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    Av1,
    Vp9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Mkv,
    Mp4,
    Webm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Medium,
    High,
    VeryHigh,
    Ultra,
}

#[derive(Debug, Clone)]
pub struct RecordSpec {
    pub source: RecordSource,
    pub fps: u32,
    pub codec: Codec,
    pub container: Container,
    pub quality: Quality,
    pub system_audio: bool,
    pub mic: Option<String>,
    pub cursor: bool,
    pub dir: PathBuf,
}

/// `segments` are `(start_us, end_us)` spans of recorded time between pauses.
#[derive(Debug, Clone)]
pub struct RecordingOutput {
    pub file: PathBuf,
    pub first_frame_monotonic_us: u64,
    pub segments: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderEvent {
    Started,
    Paused,
    Resumed,
    /// The microphone being recorded is gone from the sound server; the recording goes on.
    MicLost,
    /// The recorded window closed; the file is finished and `stop` returns it.
    WindowClosed,
    Failed(String),
}

pub trait RecordingHandle: Send {
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    /// Silences what the recording takes from `source`, while it runs.
    fn set_muted(&mut self, _source: AudioSource, _muted: bool) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn stop(self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>>;
    fn discard(self: Box<Self>) -> BoxFuture<'static, Result<()>>;
    fn events(&self) -> async_channel::Receiver<RecorderEvent>;
}
