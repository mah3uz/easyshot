use super::failure::ShareFailure;
use es_desktop::SecretsError;
use es_model::{BucketKeys, ShareCfg, ShareMode, ShareRecord};
use es_share::{Bucket, BucketConfig, Event, Free, InstallTokens, ShareError, ShareRequest};
use es_store::Paths;
use es_store::shares;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub const FREE_BASE_URL: &str = "https://share.easyshot.site";

/// Where a share goes. Every call blocks; the engine calls it on the share's own thread.
pub trait Target {
    fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<ShareRecord, ShareError>;
    fn delete(&self, record: &ShareRecord) -> Result<(), ShareError>;
}

impl Target for Free {
    fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<ShareRecord, ShareError> {
        Free::share(self, req, on_event, cancel)
    }

    fn delete(&self, record: &ShareRecord) -> Result<(), ShareError> {
        Free::delete(self, record)
    }
}

impl Target for Bucket {
    fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<ShareRecord, ShareError> {
        Bucket::share(self, req, on_event, cancel)
    }

    fn delete(&self, record: &ShareRecord) -> Result<(), ShareError> {
        Bucket::delete(self, record)
    }
}

/// Builds a target on the share's thread: reading the keyring may wait on an unlock prompt.
pub trait Targets: Send + Sync {
    fn open(&self, mode: ShareMode, cfg: &ShareCfg) -> Result<Box<dyn Target>, ShareFailure>;
}

pub trait Keyring: Send + Sync {
    /// `Err(SecretsError::Unavailable)` when no Secret Service answers.
    fn load(&self) -> Result<Option<BucketKeys>, SecretsError>;
}

pub struct SystemKeyring;

impl Keyring for SystemKeyring {
    fn load(&self) -> Result<Option<BucketKeys>, SecretsError> {
        futures::executor::block_on(es_desktop::load_bucket_keys())
    }
}

pub struct LiveTargets {
    paths: Paths,
    app_version: String,
    keyring: Arc<dyn Keyring>,
}

impl LiveTargets {
    pub fn new(paths: Paths, app_version: impl Into<String>, keyring: Arc<dyn Keyring>) -> Self {
        Self {
            paths,
            app_version: app_version.into(),
            keyring,
        }
    }

    /// An unset bucket is refused before the keyring is asked, so no unlock prompt shows for nothing.
    pub fn bucket_config(&self, cfg: &ShareCfg) -> Result<BucketConfig, ShareFailure> {
        let required = [
            &cfg.bucket_endpoint,
            &cfg.bucket_name,
            &cfg.bucket_region,
            &cfg.bucket_public_base_url,
        ];
        if required.iter().any(|v| v.trim().is_empty()) {
            return Err(ShareFailure::NotConfigured);
        }
        Ok(BucketConfig {
            endpoint: cfg.bucket_endpoint.clone(),
            bucket: cfg.bucket_name.clone(),
            region: cfg.bucket_region.clone(),
            public_base_url: cfg.bucket_public_base_url.clone(),
            prefix: cfg.bucket_prefix.clone(),
            keys: self.bucket_keys()?,
        })
    }

    /// The keyring first; keys saved while no keyring answered live in the key file.
    fn bucket_keys(&self) -> Result<BucketKeys, ShareFailure> {
        let keyring_failed = match self.keyring.load() {
            Ok(Some(keys)) => return Ok(keys),
            Ok(None) | Err(SecretsError::Unavailable) => false,
            Err(e) => {
                tracing::warn!(error = %e, "bucket keys: keyring");
                true
            }
        };
        match shares::read_bucket_keys(&self.paths) {
            Ok(Some(keys)) => Ok(keys),
            Ok(None) if keyring_failed => Err(ShareFailure::Keyring),
            Ok(None) => Err(ShareFailure::NotConfigured),
            // A parse error can quote the secret.
            Err(_) => {
                tracing::warn!("bucket keys: the key file is unreadable");
                Err(ShareFailure::NotConfigured)
            }
        }
    }
}

impl Targets for LiveTargets {
    fn open(&self, mode: ShareMode, cfg: &ShareCfg) -> Result<Box<dyn Target>, ShareFailure> {
        match mode {
            ShareMode::Free => Ok(Box::new(Free::new(
                FREE_BASE_URL,
                &self.app_version,
                StoreTokens(self.paths.clone()),
            ))),
            ShareMode::Bucket => {
                let config = self.bucket_config(cfg)?;
                match Bucket::new(&config, &self.app_version) {
                    Ok(bucket) => Ok(Box::new(bucket)),
                    Err(e) => {
                        tracing::info!(error = %e, "bucket settings");
                        Err(ShareFailure::NotConfigured)
                    }
                }
            }
        }
    }
}

struct StoreTokens(Paths);

impl InstallTokens for StoreTokens {
    fn load(&self) -> Option<String> {
        shares::read_install_token(&self.0).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "share install token");
            None
        })
    }

    fn save(&self, token: &str) {
        if let Err(e) = shares::write_install_token(&self.0, token) {
            tracing::warn!(error = %e, "share install token");
        }
    }

    fn clear(&self) {
        if let Err(e) = shares::clear_install_token(&self.0) {
            tracing::warn!(error = %e, "share install token");
        }
    }
}
