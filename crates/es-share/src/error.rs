use serde::{Deserialize, Serialize};
use std::io;

/// The share service's error codes; the app shows its own text for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    BadRequest,
    Unauthorized,
    TokenExpired,
    ChallengeFailed,
    OriginRefused,
    NotFound,
    Conflict,
    TooLarge,
    UnsupportedType,
    VersionUnsupported,
    RateLimited,
    QuotaDaily,
    Full,
}

/// What an own-bucket request found wrong, for the app to name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketFault {
    Credentials,
    Bucket,
    PublicUrl,
    Network,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("share service refused the request: {code:?}")]
    Service {
        code: Code,
        retry_after_s: Option<u64>,
    },
    #[error("network: {0}")]
    Network(String),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] io::Error),
    /// `detail` is for logs: a status and the S3 error code, never a key or a signature.
    #[error("bucket: {fault:?} ({detail})")]
    Bucket { fault: BucketFault, detail: String },
    #[error("unexpected response: {0}")]
    Protocol(String),
}

impl ShareError {
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::Service { code, .. } => Some(*code),
            _ => None,
        }
    }
}
