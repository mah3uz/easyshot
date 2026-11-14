use anyhow::{Context as _, Result};
use es_app::dispatcher::RouteDispatcher;
use es_app::events::{Origin, PlatformEvent};
use es_app::jobs::{JobFinished, JobId, Jobs};
use es_backend::{Backend, Caps};
use es_desktop::AppearanceEvent;
use es_desktop::dbus::{self, Callbacks, Endpoint, Incoming, Service};
use futures::future::{self, Either};
use futures::{StreamExt, stream};
use gpui::{App, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::pin;
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

fn callbacks(backend: &Arc<Backend>) -> Callbacks {
    let backend = backend.clone();
    Callbacks::new(Arc::new(move || {
        let doctor =
            crate::doctor::with_input(&backend.doctor_json(), crate::doctor_tools::input_here());
        let doctor = crate::doctor::with_gstreamer(&doctor, &es_record::gst_plugins::in_registry);
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
        let callers = cx
            .try_global::<super::agent::gate::Gate>()
            .map_or_else(Default::default, super::agent::gate::Gate::callers);
        let map = JobMap::default();
        let incoming = service.incoming();
        #[cfg(feature = "test-hooks")]
        let backend = backend.clone();
        let inbound_map = map.clone();
        let inbound = cx.spawn(async move |cx| {
            while let Ok(message) = incoming.recv().await {
                cx.update(|_| match message {
                    Incoming::Command { job, route, caller } => {
                        let id = jobs.allocate();
                        inbound_map.borrow_mut().insert(id, job);
                        if let Some(caller) = caller {
                            callers.borrow_mut().insert(id, caller);
                        }
                        let command = PlatformEvent::Command {
                            route,
                            origin: Origin::DBus,
                            job: id,
                        };
                        if events.try_send(command).is_err() {
                            callers.borrow_mut().remove(&id);
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

/// Tray menu, portal shortcuts, notification actions and the desktop's appearance all arrive as platform events.
pub fn platform_sources(
    backend: &Arc<Backend>,
    tray_feed: &super::tray::TrayFeed,
    events: &async_channel::Sender<PlatformEvent>,
    cx: &mut App,
) -> Vec<Task<()>> {
    let mut tasks = vec![
        follow_portal(backend, events, cx),
        forward(
            backend.notifier.actions(),
            events,
            |(id, action)| PlatformEvent::NotificationAction { id, action },
            cx,
        ),
    ];
    if let Some(tray) = &backend.tray {
        tray_feed.refresh(cx);
        tasks.push(forward(tray.activations(), events, PlatformEvent::Tray, cx));
    }
    tasks
}

pub fn live_caps(backend: &Arc<Backend>) -> Rc<dyn Fn() -> Caps> {
    let backend = backend.clone();
    Rc::new(move || backend.caps())
}

/// Shortcuts and appearance come from whichever providers the portal offers now; a provider
/// that arrives late sends the desktop's current appearance first.
fn follow_portal(
    backend: &Arc<Backend>,
    events: &async_channel::Sender<PlatformEvent>,
    cx: &App,
) -> Task<()> {
    backend.follow_portal();
    let (backend, events) = (backend.clone(), events.clone());
    let changes = backend.portal_changes();
    cx.spawn(async move |_| {
        loop {
            let shortcuts = backend.shortcuts().activations();
            let appearance = backend.appearance().map(|a| a.changes());
            let mut platform = Box::pin(stream::select(
                shortcuts.map(PlatformEvent::Shortcut),
                stream::iter(appearance)
                    .flatten()
                    .map(PlatformEvent::Appearance),
            ));
            let changed = loop {
                let event = platform.next();
                match future::select(pin!(changes.recv()), event).await {
                    Either::Left((changed, _)) => break changed.is_ok(),
                    Either::Right((Some(event), _)) => {
                        if events.send(event).await.is_err() {
                            return;
                        }
                    }
                    Either::Right((None, _)) => break changes.recv().await.is_ok(),
                }
            };
            if !changed {
                return;
            }
            if let Some(a) = backend.appearance() {
                for event in [
                    AppearanceEvent::ColorScheme(a.color_scheme()),
                    AppearanceEvent::ReduceMotion(a.reduce_motion()),
                ] {
                    if events.send(PlatformEvent::Appearance(event)).await.is_err() {
                        return;
                    }
                }
            }
        }
    })
}
