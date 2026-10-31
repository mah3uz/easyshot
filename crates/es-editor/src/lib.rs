pub mod canvas;
pub mod close_guard;
pub mod cursor;
pub mod footer;
pub mod inspector;
pub mod keyboard;
pub mod session;
pub mod toolbar;
pub mod window;

pub use session::doc::DocEvent;
pub use session::{EditorSession, EditorSource};

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
        path: std::path::PathBuf,
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
