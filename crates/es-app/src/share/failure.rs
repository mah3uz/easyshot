use crate::fl;
use crate::services::Strings;
use es_share::{BucketFault, Code, ShareError};

/// Why a share or a delete did not happen, in terms the app has its own words for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareFailure {
    NoConsent,
    NotConfigured,
    Keyring,
    Refused {
        code: Code,
        retry_after_s: Option<u64>,
    },
    Network,
    Bucket(BucketFault),
    File,
    Unexpected,
    Cancelled,
}

impl From<&ShareError> for ShareFailure {
    fn from(error: &ShareError) -> Self {
        match error {
            ShareError::Service {
                code,
                retry_after_s,
            } => Self::Refused {
                code: *code,
                retry_after_s: *retry_after_s,
            },
            ShareError::Network(_) => Self::Network,
            ShareError::Cancelled => Self::Cancelled,
            ShareError::Io(_) => Self::File,
            ShareError::Bucket { fault, .. } => Self::Bucket(*fault),
            ShareError::Protocol(_) => Self::Unexpected,
        }
    }
}

impl ShareFailure {
    pub fn id(&self) -> &'static str {
        match self {
            Self::NoConsent => "share-failed-no-consent",
            Self::NotConfigured => "share-failed-not-configured",
            Self::Keyring => "share-failed-keyring",
            Self::Network => "share-failed-network",
            Self::File => "share-failed-file",
            Self::Unexpected => "share-failed-unexpected",
            Self::Cancelled => "share-failed-cancelled",
            Self::Bucket(fault) => match fault {
                BucketFault::Credentials => "share-failed-bucket-credentials",
                BucketFault::Bucket => "share-failed-bucket-bucket",
                BucketFault::PublicUrl => "share-failed-bucket-public-url",
                BucketFault::Network => "share-failed-bucket-network",
            },
            Self::Refused { code, .. } => match code {
                Code::BadRequest => "share-failed-bad-request",
                Code::Unauthorized => "share-failed-unauthorized",
                Code::TokenExpired => "share-failed-token-expired",
                Code::ChallengeFailed => "share-failed-challenge-failed",
                Code::OriginRefused => "share-failed-origin-refused",
                Code::NotFound => "share-failed-not-found",
                Code::Conflict => "share-failed-conflict",
                Code::TooLarge => "share-failed-too-large",
                Code::UnsupportedType => "share-failed-unsupported-type",
                Code::VersionUnsupported => "share-failed-version-unsupported",
                Code::RateLimited => "share-failed-rate-limited",
                Code::QuotaDaily => "share-failed-quota-daily",
                Code::Full => "share-failed-full",
            },
        }
    }

    /// Never the service's own text: only the app's words, with the wait it asked for.
    pub fn message(&self, strings: &Strings) -> String {
        let text = strings.get(self.id());
        let Self::Refused {
            code,
            retry_after_s,
        } = self
        else {
            return text;
        };
        match retry_after_s {
            Some(secs) => format!("{text} {}", retry_in(*secs)),
            None if matches!(code, Code::RateLimited | Code::QuotaDaily | Code::Full) => {
                format!("{text} {}", fl!("share-retry-later"))
            }
            None => text,
        }
    }
}

fn retry_in(secs: u64) -> String {
    let minutes = secs.div_ceil(60).max(1);
    if minutes < 120 {
        fl!("share-retry-minutes", minutes = minutes)
    } else {
        fl!("share-retry-hours", hours = minutes.div_ceil(60))
    }
}
