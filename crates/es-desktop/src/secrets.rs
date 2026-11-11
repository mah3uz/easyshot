use es_model::BucketKeys;
use es_protocol::BUS_NAME;
use futures::FutureExt;
use oo7::dbus::{Collection, Service};
use std::time::Duration;

const KIND: &str = "bucket-keys";
const LABEL: &str = "easyshot bucket keys";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SecretsError {
    #[error("no Secret Service answers on the session bus")]
    Unavailable,
    #[error("the keyring stayed locked")]
    Locked,
    #[error("the stored bucket keys are not readable")]
    Corrupt,
    #[error("the keyring failed: {0}")]
    Failed(String),
}

impl From<oo7::dbus::Error> for SecretsError {
    fn from(e: oo7::dbus::Error) -> Self {
        match e {
            oo7::dbus::Error::Dismissed => Self::Locked,
            other => Self::Failed(other.to_string()),
        }
    }
}

pub async fn available() -> bool {
    connect().await.is_ok()
}

pub async fn load_bucket_keys() -> Result<Option<BucketKeys>, SecretsError> {
    let collection = collection().await?;
    let Some(item) = collection
        .search_items(&attributes())
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    item.unlock(None).await?;
    let secret = item.secret().await?;
    serde_json::from_slice(secret.as_bytes())
        .map(Some)
        .map_err(|_| SecretsError::Corrupt)
}

pub async fn store_bucket_keys(keys: &BucketKeys) -> Result<(), SecretsError> {
    let collection = collection().await?;
    let secret = serde_json::to_vec(keys).map_err(|_| SecretsError::Corrupt)?;
    collection
        .create_item(LABEL, &attributes(), secret, true, None)
        .await?;
    Ok(())
}

pub async fn clear_bucket_keys() -> Result<(), SecretsError> {
    let collection = collection().await?;
    for item in collection.search_items(&attributes()).await? {
        item.delete(None).await?;
    }
    Ok(())
}

fn attributes() -> [(&'static str, &'static str); 2] {
    [("application", BUS_NAME), ("kind", KIND)]
}

async fn connect() -> Result<Service, SecretsError> {
    let mut service = std::pin::pin!(Service::new().fuse());
    let mut deadline = std::pin::pin!(async_io::Timer::after(CONNECT_TIMEOUT).fuse());
    futures::select! {
        connected = service => connected.map_err(|_| SecretsError::Unavailable),
        _ = deadline => Err(SecretsError::Unavailable),
    }
}

async fn collection() -> Result<Collection, SecretsError> {
    let collection = connect().await?.default_collection().await?;
    if collection.is_locked().await? {
        collection.unlock(None).await?;
    }
    Ok(collection)
}
