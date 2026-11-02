use crate::dbus::Endpoint;
use crate::{Error, Result};
use std::sync::Arc;

pub type Labels = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub(crate) fn protocol(e: impl std::fmt::Display) -> Error {
    Error::Protocol(e.to_string())
}

pub(crate) async fn connect(endpoint: &Endpoint) -> Result<zbus::Connection> {
    endpoint
        .builder()
        .await
        .map_err(protocol)?
        .build()
        .await
        .map_err(protocol)
}

pub(crate) fn block<F: std::future::Future>(f: F) -> F::Output {
    futures::executor::block_on(f)
}
