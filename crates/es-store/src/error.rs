use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
    },
    #[error("expected format {expected}, found {found}")]
    WrongFormat { expected: String, found: String },
    #[error("created by a newer easyshot (version {version}); opened read-only")]
    NewerVersion { version: u32 },
    #[error("the file does not parse and is kept as it is until it does")]
    Damaged,
    #[error("watch error: {0}")]
    Watch(String),
    #[error(transparent)]
    Model(#[from] es_model::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}
