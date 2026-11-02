use anyhow::{Context as _, Result};
use es_app::dispatcher::RouteDispatcher;
use es_app::events::{Origin, PlatformEvent};
use es_app::history::History;
use es_app::jobs::{JobFinished, JobId, Jobs};
use es_backend::Backend;
use es_desktop::dbus::{self, Callbacks, Endpoint, Incoming, Service};
use gpui::{App, Entity, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// The bus allocates its own job ids, so each is mapped to the app's: tray and shortcut jobs
/// share the app's id space, and a bus id must never finish someone else's job.
type JobMap = Rc<RefCell<HashMap<JobId, dbus::JobId>>>;

pub struct Bridge {
    _tasks: Vec<Task<()>>,
}

fn endpoint(backend: &Backend) -> Endpoint {
    #[cfg(feature = "test-hooks")]
    if backend.test.is_some() {
        return Endpoint::test();
    }
    let _ = backend;
    Endpoint::session()
}

fn callbacks(backend: &Backend) -> Callbacks {
    let doctor = backend.doctor_json();
    Callbacks::new(Arc::new(move || {
        let doctor = doctor.clone();
        Box::pin(async move { doctor })
    }))
}

impl Bridge {
    /// Claims the bus name, which is also the single-instance lock.
    pub fn start(
        backend: &Arc<Backend>,
        events: async_channel::Sender<PlatformEvent>,
        dispatcher: RouteDispatcher,
        jobs: Jobs,
        finished: async_channel::Receiver<JobFinished>,
        cx: &mut App,
    ) -> Result<Self> {
        let service =
            futures::executor::block_on(Service::start(endpoint(backend), callbacks(backend)))
                .context("claiming the D-Bus name")?;
        let service = Arc::new(service);
        let map = JobMap::default();
        let incoming = service.incoming();
        #[cfg(feature = "test-hooks")]
        let backend = backend.clone();
        let inbound_map = map.clone();
        let inbound = cx.spawn(async move |cx| {
            while let Ok(message) = incoming.recv().await {
                cx.update(|_| match message {
                    Incoming::Command { job, route } => {
                        let id = jobs.allocate();
                        inbound_map.borrow_mut().insert(id, job);
                        let command = PlatformEvent::Command {
                            route,
                            origin: Origin::DBus,
                            job: id,
                        };
                        if events.try_send(command).is_err() {
                            tracing::warn!("event pump closed");
                        }
                    }
                    Incoming::Cancel { job } => {
                        let id = inbound_map
                            .borrow()
                            .iter()
                            .find_map(|(id, bus)| (*bus == job).then_some(*id));
                        if let Some(id) = id {
                            dispatcher.cancel(id);
                        }
                    }
                    #[cfg(feature = "test-hooks")]
                    Incoming::SetPointer { x, y } => {
                        if let Some(hooks) = &backend.test {
                            hooks.set_pointer(es_geom::Point::new(x, y));
                        }
                    }
                    #[cfg(feature = "test-hooks")]
                    Incoming::Inject(text) => {
                        let _ = events.try_send(PlatformEvent::TestInject(text));
                    }
                    other => tracing::debug!(?other, "unhandled bus message"),
                });
            }
        });
        let outbound = cx.spawn(async move |cx| {
            while let Ok(done) = finished.recv().await {
                let Some(bus_job) = map.borrow_mut().remove(&done.id) else {
                    continue;
                };
                let service = service.clone();
                let detail = done.detail.unwrap_or_default();
                cx.background_executor()
                    .spawn(async move {
                        if let Err(e) = service.finish(bus_job, done.status, &detail).await {
                            tracing::warn!(error = %e, job = bus_job, "JobFinished not sent");
                        }
                    })
                    .detach();
            }
        });
        Ok(Bridge {
            _tasks: vec![inbound, outbound],
        })
    }
}

fn forward<T: 'static>(
    source: async_channel::Receiver<T>,
    events: &async_channel::Sender<PlatformEvent>,
    wrap: fn(T) -> PlatformEvent,
    cx: &App,
) -> Task<()> {
    let events = events.clone();
    cx.spawn(async move |_| {
        while let Ok(item) = source.recv().await {
            if events.send(wrap(item)).await.is_err() {
                return;
            }
        }
    })
}

/// Tray menu, portal shortcuts and notification actions all arrive as platform events.
pub fn platform_sources(
    backend: &Arc<Backend>,
    history: &Entity<History>,
    events: &async_channel::Sender<PlatformEvent>,
    cx: &mut App,
) -> Vec<Task<()>> {
    let mut tasks = vec![
        forward(
            backend.shortcuts.activations(),
            events,
            PlatformEvent::Shortcut,
            cx,
        ),
        forward(
            backend.notifier.actions(),
            events,
            |(id, action)| PlatformEvent::NotificationAction { id, action },
            cx,
        ),
    ];
    if let Some(tray) = &backend.tray {
        let input = tray_input(history, cx);
        es_shell::tray::show(tray.as_ref(), &input);
        tasks.push(forward(tray.activations(), events, PlatformEvent::Tray, cx));
    }
    tasks
}

fn tray_input(history: &Entity<History>, cx: &App) -> es_shell::tray::TrayInput {
    let recent = history
        .read(cx)
        .entries()
        .iter()
        .take(es_shell::tray::RECENT_PER_KIND)
        .map(es_shell::tray::Recent::from)
        .collect();
    es_shell::tray::TrayInput {
        screenshots: recent,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        ..Default::default()
    }
}
