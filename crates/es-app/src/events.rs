use crate::jobs::JobId;
use es_desktop::{AppearanceEvent, NotificationId};
use es_model::platform::{OutputEvent, WindowEvent};
use es_protocol::Route;
use es_record::RecorderEvent;

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
    Command { route: Route, origin: Origin, job: JobId },
    Tray(Route),
    Shortcut(Route),
    NotificationAction { id: NotificationId, action: String },
    Appearance(AppearanceEvent),
    Outputs(OutputEvent),
    Windows(WindowEvent),
    ConfigFileChanged,
    Recorder(RecorderEvent),
    /// Test-hooks only.
    TestInject(String),
}
