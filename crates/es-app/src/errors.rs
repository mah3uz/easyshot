use crate::fl;
use crate::toast::{Toast, ToastRequest};
use es_desktop::{Notification, Urgency};
use es_protocol::JobStatus;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unsupported on this system")]
    Unsupported,
    #[error("tesseract is not installed")]
    TesseractMissing,
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
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
    },
    #[error("{0}")]
    Store(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<es_store::Error> for Error {
    fn from(e: es_store::Error) -> Self {
        match e {
            es_store::Error::Io { path, source } => Error::Io { path, source },
            es_store::Error::Parse { line, col, message } => Error::Parse { line, col, message },
            other => Error::Store(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Capture,
    Save,
    Copy,
    Ocr,
    Recording,
    Scroll,
    Open,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Toast,
    Notification,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub op: Op,
    pub surface: Surface,
}

impl Context {
    pub fn toast(op: Op) -> Self {
        Self {
            op,
            surface: Surface::Toast,
        }
    }

    pub fn notification(op: Op) -> Self {
        Self {
            op,
            surface: Surface::Notification,
        }
    }

    pub fn inline(op: Op) -> Self {
        Self {
            op,
            surface: Surface::Inline,
        }
    }
}

pub enum Presentation {
    Toast(Toast),
    Notification(Notification),
    Inline(String),
    Silent,
}

pub fn title_id(error: &Error, op: Op) -> &'static str {
    match (error, op) {
        (Error::Unsupported, _) => "toast-unsupported",
        (Error::TesseractMissing, _) => "toast-ocr-missing",
        (Error::NotFound(_), Op::Open) => "toast-capture-unavailable",
        (_, Op::Capture) => "toast-screenshot-failed",
        (_, Op::Save) => "toast-capture-save-failed",
        (_, Op::Copy) => "toast-copy-failed",
        (_, Op::Ocr) => "toast-ocr-failed",
        (_, Op::Recording) => "toast-recording-save-failed",
        (_, Op::Scroll) => "toast-scroll-capture-failed",
        (_, Op::Open | Op::Other) => "toast-error-generic",
    }
}

pub fn recording_start_title(error: &Error) -> &'static str {
    match error {
        Error::AudioDeviceMissing => "toast-recording-no-audio-device",
        Error::AreaOffMonitor => "toast-recording-area-off-monitor",
        _ => "toast-error-generic",
    }
}

fn notification(title: String, body: String) -> Notification {
    Notification {
        title,
        body,
        icon: None,
        image: None,
        actions: Vec::new(),
        urgency: Urgency::Normal,
        timeout: None,
    }
}

pub fn present(error: &Error, cx: Context) -> Presentation {
    if matches!(error, Error::Cancelled) {
        return Presentation::Silent;
    }
    if let Error::Parse { line, col, message } = error {
        return Presentation::Notification(notification(
            fl!("notify-config-invalid"),
            fl!(
                "notify-config-invalid-body",
                line = line.to_string(),
                col = col.to_string(),
                message = message.clone()
            ),
        ));
    }
    let toast = Toast::error(title_id(error, cx.op)).arg("error", error.to_string());
    match cx.surface {
        Surface::Toast => Presentation::Toast(toast),
        Surface::Notification => {
            let request = ToastRequest::resolve(&toast);
            Presentation::Notification(notification(
                request.title,
                request.message.unwrap_or_default(),
            ))
        }
        Surface::Inline => {
            let request = ToastRequest::resolve(&toast);
            Presentation::Inline(match request.message {
                Some(m) => format!("{} {m}", request.title),
                None => request.title,
            })
        }
    }
}

pub fn status_of<T>(result: &Result<T>) -> JobStatus {
    match result {
        Ok(_) => JobStatus::Ok,
        Err(Error::Cancelled) => JobStatus::Cancelled,
        Err(_) => JobStatus::Error,
    }
}
