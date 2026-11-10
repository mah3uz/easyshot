use super::{ClientError, Endpoint, JobId, sleep};
use es_protocol::{ArgValue, INTERFACE, JobStatus, OBJECT_PATH, RouteArgs};
use futures::{FutureExt, StreamExt, select};
use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::time::{Duration, Instant};
use zbus::zvariant::Value;
use zbus::{Connection, Proxy, fdo};

const POLL: Duration = Duration::from_millis(50);

pub struct Launch {
    pub spawn: Option<std::process::Command>,
    pub wait: Duration,
}

impl Default for Launch {
    fn default() -> Self {
        Launch {
            spawn: None,
            wait: Duration::from_secs(3),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    pub job: JobId,
    pub status: JobStatus,
    pub result: String,
}

pub struct Client {
    conn: Connection,
    name: &'static str,
}

impl Client {
    pub async fn connect(endpoint: &Endpoint, launch: Launch) -> Result<Client, ClientError> {
        let conn = endpoint.builder().await?.build().await?;
        let client = Client {
            conn,
            name: endpoint.name,
        };
        client.ensure_daemon(launch).await?;
        Ok(client)
    }

    /// Connects only to a daemon that is already running: it never activates or starts one.
    pub async fn connect_running(endpoint: &Endpoint) -> Result<Client, ClientError> {
        let conn = endpoint.builder().await?.build().await?;
        let client = Client {
            conn,
            name: endpoint.name,
        };
        let dbus = fdo::DBusProxy::new(&client.conn).await?;
        let name = zbus::names::BusName::try_from(client.name).map_err(zbus::Error::from)?;
        if !dbus.name_has_owner(name).await? {
            return Err(ClientError::DaemonUnavailable);
        }
        Ok(client)
    }

    async fn ensure_daemon(&self, launch: Launch) -> Result<(), ClientError> {
        let dbus = fdo::DBusProxy::new(&self.conn).await?;
        let name = zbus::names::BusName::try_from(self.name).map_err(zbus::Error::from)?;
        if dbus.name_has_owner(name.clone()).await? {
            return Ok(());
        }
        let wk = zbus::names::WellKnownName::try_from(self.name).map_err(zbus::Error::from)?;
        let activated = dbus.start_service_by_name(wk, 0).await.is_ok();
        if !activated && let Some(mut cmd) = launch.spawn {
            cmd.spawn().map_err(ClientError::Spawn)?;
        }
        let deadline = Instant::now() + launch.wait;
        while !dbus.name_has_owner(name.clone()).await? {
            if Instant::now() >= deadline {
                return Err(ClientError::DaemonUnavailable);
            }
            sleep(POLL).await;
        }
        Ok(())
    }

    async fn proxy(&self) -> Result<Proxy<'static>, ClientError> {
        Ok(Proxy::new(&self.conn, self.name, OBJECT_PATH, INTERFACE).await?)
    }

    pub async fn doctor(&self) -> Result<String, ClientError> {
        let proxy = self.proxy().await?;
        proxy.call("Doctor", &()).await.map_err(map_call_error)
    }

    /// Starts the job and returns its id without waiting for `JobFinished`.
    pub async fn submit(&self, route: &str, args: &RouteArgs) -> Result<JobId, ClientError> {
        let proxy = self.proxy().await?;
        proxy
            .call("Command", &(route, wire_args(args)?))
            .await
            .map_err(map_call_error)
    }

    pub async fn run(
        &self,
        route: &str,
        args: &RouteArgs,
        cancel: impl Future<Output = ()>,
    ) -> Result<JobOutcome, ClientError> {
        let proxy = self.proxy().await?;
        let dbus = fdo::DBusProxy::new(&self.conn).await?;
        let mut gone = dbus
            .receive_name_owner_changed_with_args(&[(0, self.name)])
            .await?
            .fuse();
        let mut finished = proxy.receive_signal("JobFinished").await?.fuse();
        let mut cancel = pin!(cancel.fuse());

        let job: JobId = proxy
            .call("Command", &(route, wire_args(args)?))
            .await
            .map_err(map_call_error)?;

        loop {
            select! {
                msg = finished.next() => {
                    let Some(msg) = msg else { return Err(ClientError::DaemonGone) };
                    let (id, status, result): (u64, String, String) =
                        msg.body().deserialize()?;
                    if id == job {
                        return Ok(JobOutcome { job, status: parse_status(&status), result });
                    }
                }
                change = gone.next() => {
                    let vanished = match change {
                        Some(c) => c.args()?.new_owner().is_none(),
                        None => true,
                    };
                    if vanished {
                        return Err(ClientError::DaemonGone);
                    }
                }
                () = cancel => {
                    proxy.call_method("Cancel", &(job,)).await.map_err(map_call_error)?;
                }
            }
        }
    }
}

fn parse_status(s: &str) -> JobStatus {
    match s {
        "ok" => JobStatus::Ok,
        "cancelled" => JobStatus::Cancelled,
        _ => JobStatus::Error,
    }
}

fn wire_args(args: &RouteArgs) -> Result<HashMap<String, Value<'static>>, ClientError> {
    args.0
        .iter()
        .map(|(k, v)| {
            let value = match v {
                ArgValue::Str(s) => Value::from(s.clone()),
                ArgValue::Bool(b) => Value::from(*b),
                ArgValue::Int(n) => Value::from(*n),
                ArgValue::Path(p) => Value::from(
                    p.to_str()
                        .ok_or_else(|| ClientError::Rejected(format!("{k}: path is not UTF-8")))?
                        .to_owned(),
                ),
            };
            Ok((k.clone(), value))
        })
        .collect()
}

fn map_call_error(e: zbus::Error) -> ClientError {
    match e {
        zbus::Error::MethodError(_, Some(msg), _) => ClientError::Rejected(msg),
        zbus::Error::FDO(e) => ClientError::Rejected(e.to_string()),
        other => ClientError::Bus(other),
    }
}
