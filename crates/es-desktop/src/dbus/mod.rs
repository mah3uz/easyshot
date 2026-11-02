mod client;
mod service;
#[cfg(feature = "test-hooks")]
mod test_iface;

pub use client::{Client, JobOutcome, Launch};
#[cfg(feature = "test-hooks")]
pub use service::DumpFn;
pub use service::{Callbacks, DoctorFn, Service};

use es_protocol::{BUS_NAME, Route};
use std::time::Duration;

pub type JobId = u64;

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub(crate) address: Option<String>,
    pub(crate) name: &'static str,
}

impl Endpoint {
    pub fn session() -> Self {
        Endpoint {
            address: None,
            name: BUS_NAME,
        }
    }

    #[cfg(feature = "test-hooks")]
    pub fn test() -> Self {
        Endpoint {
            address: None,
            name: es_protocol::BUS_NAME_TEST,
        }
    }

    pub fn on_address(mut self, address: impl Into<String>) -> Self {
        self.address = Some(address.into());
        self
    }

    pub(crate) async fn builder(&self) -> zbus::Result<zbus::connection::Builder<'static>> {
        match &self.address {
            Some(a) => zbus::connection::Builder::address(a.as_str()),
            None => zbus::connection::Builder::session(),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Incoming {
    Command {
        job: JobId,
        route: Route,
    },
    Cancel {
        job: JobId,
    },
    #[cfg(feature = "test-hooks")]
    Inject(String),
    #[cfg(feature = "test-hooks")]
    SetPointer {
        x: f64,
        y: f64,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ServiceError {
    #[error("another easyshot instance owns the bus name")]
    AlreadyRunning,
    #[error("unknown job {0}")]
    UnknownJob(JobId),
    #[error("bus error: {0}")]
    Bus(#[from] zbus::Error),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("the daemon went away")]
    DaemonGone,
    #[error("the daemon did not appear on the bus")]
    DaemonUnavailable,
    #[error("could not start the daemon: {0}")]
    Spawn(std::io::Error),
    #[error("{0}")]
    Rejected(String),
    #[error("bus error: {0}")]
    Bus(#[from] zbus::Error),
    #[error("bus error: {0}")]
    Fdo(#[from] zbus::fdo::Error),
}

impl ClientError {
    pub fn exit_code(&self) -> i32 {
        match self {
            ClientError::DaemonGone => 3,
            _ => 1,
        }
    }
}

pub(crate) async fn sleep(d: Duration) {
    let (tx, rx) = async_channel::bounded::<()>(1);
    std::thread::spawn(move || {
        std::thread::sleep(d);
        let _ = tx.send_blocking(());
    });
    let _ = rx.recv().await;
}
