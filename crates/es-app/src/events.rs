use crate::dispatcher::RouteDispatcher;
use crate::jobs::JobId;
use crate::settings_model::SettingsModel;
use es_desktop::{AppearanceEvent, NotificationId};
use es_model::platform::{OutputEvent, WindowEvent};
use es_protocol::Route;
use es_record::RecorderEvent;
use gpui::{App, Entity, Task};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    DBus,
    Tray,
    Shortcut,
    Url,
    Internal,
}

#[derive(Debug, Clone)]
pub enum PlatformEvent {
    Command {
        route: Route,
        origin: Origin,
        job: JobId,
    },
    Tray(Route),
    Shortcut(Route),
    NotificationAction {
        id: NotificationId,
        action: String,
    },
    Appearance(AppearanceEvent),
    Outputs(OutputEvent),
    Windows(WindowEvent),
    ConfigFileChanged,
    Recorder(RecorderEvent),
    /// Test-hooks only.
    TestInject(String),
}

type Handler<T> = Rc<dyn Fn(T, &mut App)>;

/// Events the app answers itself; an event with no handler is dropped.
#[derive(Default, Clone)]
pub struct EventHandlers {
    pub notification_action: Option<Handler<(NotificationId, String)>>,
    pub appearance: Option<Handler<AppearanceEvent>>,
    pub outputs: Option<Handler<OutputEvent>>,
    pub windows: Option<Handler<WindowEvent>>,
    pub recorder: Option<Handler<RecorderEvent>>,
    pub test_inject: Option<Handler<String>>,
}

pub fn handle_event(
    event: PlatformEvent,
    dispatcher: &RouteDispatcher,
    settings: &Entity<SettingsModel>,
    handlers: &EventHandlers,
    cx: &mut App,
) {
    fn forward<T>(handler: &Option<Handler<T>>, value: T, cx: &mut App) {
        if let Some(handler) = handler {
            handler(value, cx);
        }
    }
    match event {
        PlatformEvent::Command { route, origin, job } => {
            dispatcher.dispatch_as(job, route, origin, cx)
        }
        PlatformEvent::Tray(route) => {
            dispatcher.dispatch(route, Origin::Tray, cx);
        }
        PlatformEvent::Shortcut(route) => {
            dispatcher.dispatch(route, Origin::Shortcut, cx);
        }
        PlatformEvent::NotificationAction { id, action } => {
            forward(&handlers.notification_action, (id, action), cx)
        }
        PlatformEvent::Appearance(e) => forward(&handlers.appearance, e, cx),
        PlatformEvent::Outputs(e) => forward(&handlers.outputs, e, cx),
        PlatformEvent::Windows(e) => forward(&handlers.windows, e, cx),
        PlatformEvent::ConfigFileChanged => {
            settings.update(cx, |settings, cx| settings.reload(cx));
        }
        PlatformEvent::Recorder(e) => forward(&handlers.recorder, e, cx),
        PlatformEvent::TestInject(text) => forward(&handlers.test_inject, text, cx),
    }
}

/// Drains `events` on the GPUI thread until the channel closes; dropping the task stops it.
pub fn spawn_pump(
    events: async_channel::Receiver<PlatformEvent>,
    dispatcher: RouteDispatcher,
    settings: Entity<SettingsModel>,
    handlers: EventHandlers,
    cx: &App,
) -> Task<()> {
    cx.spawn(async move |cx| {
        while let Ok(event) = events.recv().await {
            cx.update(|cx| handle_event(event, &dispatcher, &settings, &handlers, cx));
        }
    })
}
