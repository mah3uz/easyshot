use super::{Endpoint, Incoming, JobId, ServiceError};
use es_protocol::{ArgValue, JobStatus, OBJECT_PATH, Route, RouteArgs};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use zbus::fdo::{self, RequestNameFlags, RequestNameReply};
use zbus::message::Header;
use zbus::names::{BusName, OwnedUniqueName};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, object_server::SignalEmitter};

pub type DoctorFn = Arc<dyn Fn() -> BoxFuture<'static, String> + Send + Sync>;
#[cfg(feature = "test-hooks")]
pub type DumpFn = Arc<dyn Fn() -> BoxFuture<'static, String> + Send + Sync>;

#[derive(Clone)]
pub struct Callbacks {
    pub doctor: DoctorFn,
    #[cfg(feature = "test-hooks")]
    pub dump: DumpFn,
}

impl Callbacks {
    /// The test dump, when built in, answers with an empty object.
    pub fn new(doctor: DoctorFn) -> Self {
        Callbacks {
            doctor,
            #[cfg(feature = "test-hooks")]
            dump: Arc::new(|| Box::pin(async { "{}".to_owned() })),
        }
    }
}

type Live = Arc<Mutex<HashMap<JobId, OwnedUniqueName>>>;

fn lock(live: &Live) -> std::sync::MutexGuard<'_, HashMap<JobId, OwnedUniqueName>> {
    live.lock().unwrap_or_else(PoisonError::into_inner)
}

struct CommandIface {
    next: AtomicU64,
    live: Live,
    tx: async_channel::Sender<Incoming>,
    doctor: DoctorFn,
}

#[zbus::interface(name = "dev.easyshot.app")]
impl CommandIface {
    async fn command(
        &self,
        #[zbus(header)] header: Header<'_>,
        route: String,
        args: HashMap<String, OwnedValue>,
    ) -> fdo::Result<u64> {
        let caller = sender(&header)?;
        let parsed = parse_off_the_bus(route, args).await?;
        let job = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.live).insert(job, caller);
        let route = Incoming::Command { job, route: parsed };
        if self.tx.send(route).await.is_err() {
            lock(&self.live).remove(&job);
            return Err(fdo::Error::Failed("the app is shutting down".into()));
        }
        Ok(job)
    }

    async fn cancel(&self, #[zbus(header)] header: Header<'_>, job_id: u64) -> fdo::Result<()> {
        let caller = sender(&header)?;
        if lock(&self.live).get(&job_id) != Some(&caller) {
            return Err(fdo::Error::InvalidArgs(format!("unknown job {job_id}")));
        }
        self.tx
            .send(Incoming::Cancel { job: job_id })
            .await
            .map_err(|_| fdo::Error::Failed("the app is shutting down".into()))
    }

    async fn doctor(&self) -> String {
        (self.doctor)().await
    }

    #[zbus(signal)]
    async fn job_finished(
        emitter: &SignalEmitter<'_>,
        job_id: u64,
        status: &str,
        result: &str,
    ) -> zbus::Result<()>;
}

fn sender(header: &Header<'_>) -> fdo::Result<OwnedUniqueName> {
    header
        .sender()
        .map(|s| s.to_owned().into())
        .ok_or_else(|| fdo::Error::AccessDenied("a command needs a sender".into()))
}

/// `open` touches the file system; a slow disk must not stall the bus executor.
async fn parse_off_the_bus(route: String, args: HashMap<String, OwnedValue>) -> fdo::Result<Route> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(parse_route(&route, &args));
    });
    rx.recv()
        .await
        .map_err(|_| fdo::Error::Failed("the route could not be checked".into()))?
}

fn parse_route(route: &str, args: &HashMap<String, OwnedValue>) -> fdo::Result<Route> {
    let mut parsed_args = RouteArgs::default();
    for (key, value) in args {
        let v = match &**value {
            Value::Str(s) => ArgValue::Str(s.to_string()),
            Value::Bool(b) => ArgValue::Bool(*b),
            Value::I64(n) => ArgValue::Int(*n),
            Value::I32(n) => ArgValue::Int(i64::from(*n)),
            Value::U32(n) => ArgValue::Int(i64::from(*n)),
            _ => {
                return Err(fdo::Error::InvalidArgs(format!(
                    "argument {key} has an unsupported type"
                )));
            }
        };
        parsed_args.0.insert(key.clone(), v);
    }
    Route::parse(route, &parsed_args).map_err(|e| fdo::Error::InvalidArgs(e.to_string()))
}

pub struct Service {
    conn: Connection,
    incoming: async_channel::Receiver<Incoming>,
    live: Live,
}

impl Service {
    pub async fn start(endpoint: Endpoint, callbacks: Callbacks) -> Result<Service, ServiceError> {
        let (tx, rx) = async_channel::unbounded();
        let live: Live = Arc::default();
        let iface = CommandIface {
            next: AtomicU64::new(1),
            live: live.clone(),
            tx: tx.clone(),
            doctor: callbacks.doctor.clone(),
        };
        let builder = endpoint.builder().await?.serve_at(OBJECT_PATH, iface)?;
        #[cfg(feature = "test-hooks")]
        let builder = if endpoint.name == es_protocol::BUS_NAME_TEST {
            builder.serve_at(
                OBJECT_PATH,
                super::test_iface::TestIface::new(tx, callbacks.dump.clone()),
            )?
        } else {
            builder
        };
        let conn = builder.build().await?;
        let reply = conn
            .request_name_with_flags(endpoint.name, RequestNameFlags::DoNotQueue.into())
            .await;
        match reply {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => {}
            Ok(_) | Err(zbus::Error::NameTaken) => return Err(ServiceError::AlreadyRunning),
            Err(e) => return Err(e.into()),
        }
        Ok(Service {
            conn,
            incoming: rx,
            live,
        })
    }

    pub fn incoming(&self) -> async_channel::Receiver<Incoming> {
        self.incoming.clone()
    }

    pub async fn finish(
        &self,
        job: JobId,
        status: JobStatus,
        result: &str,
    ) -> Result<(), ServiceError> {
        let Some(caller) = lock(&self.live).remove(&job) else {
            return Err(ServiceError::UnknownJob(job));
        };
        let emitter = SignalEmitter::new(&self.conn, OBJECT_PATH)?
            .set_destination(BusName::Unique(caller.into()));
        CommandIface::job_finished(&emitter, job, status.as_str(), result).await?;
        Ok(())
    }

    pub async fn close(self) {
        self.conn.graceful_shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interface_name_matches_the_protocol_constant() {
        use zbus::object_server::Interface;
        assert_eq!(CommandIface::name(), es_protocol::INTERFACE);
    }
}
