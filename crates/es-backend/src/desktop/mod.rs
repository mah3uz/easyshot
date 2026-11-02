use crate::error::BackendError;
use crate::probe::Provider;
use crate::traits::Capture;
use es_desktop::{Appearance, FilePicker, Notification, NotificationId, Notifier, Shortcuts, Tray};
use es_protocol::{Route, ShortcutEntry};
use es_record::Recorder;

fn no_client<T: ?Sized>(what: &str) -> Result<Box<T>, BackendError> {
    Err(BackendError::Unavailable(format!(
        "this build has no {what} client"
    )))
}

pub const PORTAL_SCREENSHOT: Provider<dyn Capture> = Provider {
    name: "portal-screenshot",
    probe: |p| p.portals.screenshot.then_some(1),
    build: |_| no_client("portal Screenshot"),
};

pub const PORTAL_GLOBAL_SHORTCUTS: Provider<dyn Shortcuts> = Provider {
    name: "portal-global-shortcuts",
    probe: |p| p.portals.global_shortcuts.then_some(10),
    build: |_| no_client("portal GlobalShortcuts"),
};

/// Shortcuts bound in the compositor's own config, which runs `easyshot <command>`.
pub struct UserBinds {
    _keep_open: async_channel::Sender<Route>,
    activations: async_channel::Receiver<Route>,
}

impl UserBinds {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        UserBinds {
            _keep_open: tx,
            activations: rx,
        }
    }
}

impl Default for UserBinds {
    fn default() -> Self {
        Self::new()
    }
}

impl Shortcuts for UserBinds {
    fn register(&self, _defs: &[ShortcutEntry]) -> es_desktop::Result<()> {
        Ok(())
    }
    fn activations(&self) -> async_channel::Receiver<Route> {
        self.activations.clone()
    }
    fn user_binds_required(&self) -> bool {
        true
    }
}

pub const USER_BINDS: Provider<dyn Shortcuts> = Provider {
    name: "user-binds",
    probe: |_| Some(1),
    build: |_| Ok(Box::new(UserBinds::new())),
};

pub const STATUS_NOTIFIER: Provider<dyn Tray> = Provider {
    name: "status-notifier",
    probe: |p| p.dbus.has("org.kde.StatusNotifierWatcher").then_some(10),
    build: |_| no_client("StatusNotifierItem"),
};

pub const FREEDESKTOP_NOTIFICATIONS: Provider<dyn Notifier> = Provider {
    name: "freedesktop-notifications",
    probe: |p| p.dbus.has("org.freedesktop.Notifications").then_some(10),
    build: |_| no_client("notifications"),
};

pub const PORTAL_FILE_CHOOSER: Provider<dyn FilePicker> = Provider {
    name: "portal-file-chooser",
    probe: |p| p.portals.file_chooser.then_some(10),
    build: |_| no_client("portal FileChooser"),
};

pub const PORTAL_SETTINGS: Provider<dyn Appearance> = Provider {
    name: "portal-settings",
    probe: |p| p.portals.settings.then_some(10),
    build: |_| no_client("portal Settings"),
};

pub const PORTAL_RECORDER: Provider<dyn Recorder> = Provider {
    name: "portal-recorder",
    probe: |p| (p.portals.screencast && p.tools.has("pipewiresrc")).then_some(1),
    build: |_| no_client("portal ScreenCast recorder"),
};

pub struct NoNotifier {
    _keep_open: async_channel::Sender<(NotificationId, String)>,
    actions: async_channel::Receiver<(NotificationId, String)>,
}

impl NoNotifier {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        NoNotifier {
            _keep_open: tx,
            actions: rx,
        }
    }
}

impl Default for NoNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for NoNotifier {
    fn notify(&self, _n: Notification) -> es_desktop::Result<NotificationId> {
        Err(es_desktop::Error::Unsupported)
    }
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        self.actions.clone()
    }
    fn supports_actions(&self) -> bool {
        false
    }
}
