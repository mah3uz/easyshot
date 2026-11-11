#![allow(clippy::unwrap_used)]
use es_app::services::Strings;
use es_app::share::{KeyPlace, Keyring, LiveTargets, ShareFailure, Targets};
use es_desktop::SecretsError;
use es_model::{BucketKeys, ShareCfg, ShareMode};
use es_share::{BucketFault, Code, ShareError};
use es_store::Paths;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn every_error_maps_to_a_failure_with_its_own_words() {
    let cases = [
        (
            ShareError::Network("dns: secret-host.internal".into()),
            ShareFailure::Network,
        ),
        (ShareError::Cancelled, ShareFailure::Cancelled),
        (
            ShareError::Io(std::io::Error::other("gone")),
            ShareFailure::File,
        ),
        (
            ShareError::Bucket {
                fault: BucketFault::Credentials,
                detail: "403 SignatureDoesNotMatch".into(),
            },
            ShareFailure::Bucket(BucketFault::Credentials),
        ),
        (
            ShareError::Protocol("share id".into()),
            ShareFailure::Unexpected,
        ),
        (
            ShareError::Service {
                code: Code::Full,
                retry_after_s: None,
            },
            ShareFailure::Refused {
                code: Code::Full,
                retry_after_s: None,
            },
        ),
    ];
    for (error, want) in cases {
        let got = ShareFailure::from(&error);
        assert_eq!(got, want, "{error}");
        let text = got.message(&Strings);
        assert!(
            !text.contains("secret-host") && !text.contains("Signature"),
            "{text}"
        );
    }
    assert_eq!(
        ShareFailure::Refused {
            code: Code::QuotaDaily,
            retry_after_s: Some(6 * 3600),
        }
        .message(&Strings),
        "You have shared as much as the service allows today. Try again in 6 hours."
    );
    assert_eq!(
        ShareFailure::Refused {
            code: Code::Full,
            retry_after_s: None,
        }
        .message(&Strings),
        "The share service is full right now. Try again later."
    );
    let codes = [
        Code::BadRequest,
        Code::Unauthorized,
        Code::TokenExpired,
        Code::ChallengeFailed,
        Code::OriginRefused,
        Code::NotFound,
        Code::Conflict,
        Code::TooLarge,
        Code::UnsupportedType,
        Code::VersionUnsupported,
        Code::RateLimited,
        Code::QuotaDaily,
        Code::Full,
    ];
    let faults = [
        BucketFault::Credentials,
        BucketFault::Bucket,
        BucketFault::PublicUrl,
        BucketFault::Network,
    ];
    let all = [
        ShareFailure::NoConsent,
        ShareFailure::NotConfigured,
        ShareFailure::Keyring,
        ShareFailure::Network,
        ShareFailure::File,
        ShareFailure::Unexpected,
        ShareFailure::Cancelled,
    ]
    .into_iter()
    .chain(codes.map(|code| ShareFailure::Refused {
        code,
        retry_after_s: None,
    }))
    .chain(faults.map(ShareFailure::Bucket));
    let mut ids = Vec::new();
    for failure in all {
        assert!(
            Strings.has(failure.id()),
            "{failure:?}: {} is missing",
            failure.id()
        );
        ids.push(failure.id());
    }
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "two failures share one message");
}

struct FakeKeyring {
    asked: AtomicUsize,
    answer: fn() -> std::result::Result<Option<BucketKeys>, SecretsError>,
}

impl Keyring for FakeKeyring {
    fn load(&self) -> std::result::Result<Option<BucketKeys>, SecretsError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        (self.answer)()
    }

    fn store(&self, _: &BucketKeys) -> std::result::Result<(), SecretsError> {
        (self.answer)().map(|_| ())
    }

    fn clear(&self) -> std::result::Result<(), SecretsError> {
        (self.answer)().map(|_| ())
    }
}

#[test]
fn keys_saved_to_an_answering_keyring_leave_no_copy_in_the_key_file() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let old = BucketKeys {
        access_key_id: "AKIA-old".into(),
        secret_access_key: "old-secret".into(),
    };
    es_store::shares::write_bucket_keys(&paths, old.clone()).unwrap();
    let keyring = Arc::new(FakeKeyring {
        asked: AtomicUsize::new(0),
        answer: || Ok(None),
    });
    let targets = LiveTargets::new(paths.clone(), "1.0.0", keyring);
    assert_eq!(targets.save_keys(&old), Ok(KeyPlace::Keyring));
    assert_eq!(es_store::shares::read_bucket_keys(&paths).unwrap(), None);

    let locked = Arc::new(FakeKeyring {
        asked: AtomicUsize::new(0),
        answer: || Err(SecretsError::Locked),
    });
    let targets = LiveTargets::new(paths.clone(), "1.0.0", locked);
    assert_eq!(
        targets.save_keys(&old),
        Err(ShareFailure::Keyring),
        "a keyring that stays locked must not send the keys to a file"
    );
    assert_eq!(es_store::shares::read_bucket_keys(&paths).unwrap(), None);
}

fn bucket_cfg() -> ShareCfg {
    ShareCfg {
        target: ShareMode::Bucket,
        consented: true,
        bucket_endpoint: "https://s3.example".into(),
        bucket_name: "shots".into(),
        bucket_region: "eu-1".into(),
        bucket_public_base_url: "https://cdn.example".into(),
        ..ShareCfg::default()
    }
}

#[test]
fn bucket_keys_come_from_the_key_file_when_no_keyring_answers() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let keyring = |answer| {
        Arc::new(FakeKeyring {
            asked: AtomicUsize::new(0),
            answer,
        })
    };
    let none = keyring(|| Err(SecretsError::Unavailable));
    let targets = LiveTargets::new(paths.clone(), "1.0.0", none.clone());
    assert_eq!(
        targets.bucket_config(&bucket_cfg()).map(|c| c.keys),
        Err(ShareFailure::NotConfigured)
    );
    let keys = BucketKeys {
        access_key_id: "AKIA-file".into(),
        secret_access_key: "from-the-file".into(),
    };
    es_store::shares::write_bucket_keys(&paths, keys.clone()).unwrap();
    assert_eq!(
        targets.bucket_config(&bucket_cfg()).map(|c| c.keys),
        Ok(keys)
    );

    let locked = keyring(|| Err(SecretsError::Locked));
    es_store::shares::clear_bucket_keys(&paths).unwrap();
    let targets = LiveTargets::new(paths.clone(), "1.0.0", locked.clone());
    assert_eq!(
        targets.bucket_config(&bucket_cfg()).map(|c| c.keys),
        Err(ShareFailure::Keyring)
    );

    let unset = ShareCfg {
        bucket_endpoint: String::new(),
        ..bucket_cfg()
    };
    let asked = locked.asked.load(Ordering::SeqCst);
    assert_eq!(
        targets.bucket_config(&unset).map(|c| c.keys),
        Err(ShareFailure::NotConfigured)
    );
    assert_eq!(
        locked.asked.load(Ordering::SeqCst),
        asked,
        "an unset bucket must not raise a keyring prompt"
    );
}
