use es_geom::Rect;
use es_model::platform::{OutputId, WindowInfo};
use futures::future::BoxFuture;
use std::path::PathBuf;

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
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse { line: usize, col: usize, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Recorder: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self) -> RecorderProbe;
    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderProbe {
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RecordSource {
    Output(OutputId),
    Region { output: OutputId, logical: Rect },
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
    Failed(String),
}

pub trait RecordingHandle: Send {
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>>;
    fn discard(self: Box<Self>) -> BoxFuture<'static, Result<()>>;
    fn events(&self) -> async_channel::Receiver<RecorderEvent>;
}
