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
    Parse {
        line: usize,
        col: usize,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BackendError {
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    #[error("test mode needs EASYSHOT_TEST_OUTPUT")]
    NoTestOutput,
    #[error(transparent)]
    Backend(#[from] Error),
}

impl From<es_wayland::Error> for Error {
    fn from(e: es_wayland::Error) -> Self {
        match e {
            es_wayland::Error::Unsupported(_) => Error::Unsupported,
            es_wayland::Error::Timeout => Error::Timeout,
            es_wayland::Error::UnknownOutput(id) | es_wayland::Error::UnknownToplevel(id) => {
                Error::NotFound(id)
            }
            other => Error::Protocol(other.to_string()),
        }
    }
}
