use crate::error::BackendError;
use crate::probe::{Probe, Provider};
use crate::traits::Capture;
use es_desktop::dbus::Endpoint;
use es_desktop::{
    Appearance, DesktopNotifier, FilePicker, KsniTray, Labels, Notification, NotificationId,
    Notifier, PortalAppearance, PortalFilePicker, PortalShortcuts, RecordingNotifier,
    RecordingShortcuts, RecordingTray, Shortcuts, Tray,
};
use es_protocol::{Route, ShortcutEntry};
use es_record::Recorder;
use std::sync::Arc;

#[cfg(feature = "test-hooks")]
fn test_mode(p: &Probe) -> bool {
    p.test.is_some()
}

#[cfg(not(feature = "test-hooks"))]
fn test_mode(_: &Probe) -> bool {
    false
}

fn unavailable(what: &str, e: &es_desktop::Error) -> BackendError {
    BackendError::Unavailable(format!("{what}: {e}"))
}

fn not_in_test_mode<T: ?Sized>(what: &str) -> Result<Box<T>, BackendError> {
    Err(BackendError::Unavailable(format!(
        "test mode opens no {what}"
    )))
}

// The Fluent catalogue sits above this layer; the app passes its lookup in the probe.
fn labels(p: &Probe) -> Labels {
    p.labels
        .clone()
        .unwrap_or_else(|| Arc::new(|id: &str| id.to_owned()))
}

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
    build: |p| {
        if test_mode(p) {
            return Ok(Box::new(RecordingShortcuts::new(false)));
        }
        Ok(Box::new(futures::executor::block_on(
            PortalShortcuts::start(&Endpoint::session(), labels(p)),
        )))
    },
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
    build: |p| {
        if test_mode(p) {
            return Ok(Box::new(RecordingTray::default()));
        }
        futures::executor::block_on(KsniTray::start(labels(p), p.tray_icon.clone()))
            .map(|t| Box::new(t) as Box<dyn Tray>)
            .map_err(|e| unavailable("tray", &e))
    },
};

pub const FREEDESKTOP_NOTIFICATIONS: Provider<dyn Notifier> = Provider {
    name: "freedesktop-notifications",
    probe: |p| p.dbus.has("org.freedesktop.Notifications").then_some(10),
    build: |p| {
        if test_mode(p) {
            return Ok(Box::new(RecordingNotifier::new(false)));
        }
        futures::executor::block_on(DesktopNotifier::connect(&Endpoint::session()))
            .map(|n| Box::new(n) as Box<dyn Notifier>)
            .map_err(|e| unavailable("notifications", &e))
    },
};

pub const PORTAL_FILE_CHOOSER: Provider<dyn FilePicker> = Provider {
    name: "portal-file-chooser",
    probe: |p| p.portals.file_chooser.then_some(10),
    build: |p| {
        if test_mode(p) {
            return not_in_test_mode("file dialog");
        }
        Ok(Box::new(PortalFilePicker::new(Endpoint::session())))
    },
};

pub const PORTAL_SETTINGS: Provider<dyn Appearance> = Provider {
    name: "portal-settings",
    probe: |p| p.portals.settings.then_some(10),
    build: |p| {
        if test_mode(p) {
            return not_in_test_mode("portal Settings");
        }
        futures::executor::block_on(PortalAppearance::start(&Endpoint::session()))
            .map(|a| Box::new(a) as Box<dyn Appearance>)
            .map_err(|e| unavailable("portal Settings", &e))
    },
};

fn usable(recorder: impl Recorder + 'static) -> Result<Box<dyn Recorder>, BackendError> {
    let probe = recorder.probe();
    if probe.available {
        Ok(Box::new(recorder))
    } else {
        Err(BackendError::Unavailable(
            probe.reason.unwrap_or_else(|| recorder.name().to_owned()),
        ))
    }
}

pub const GSR_NATIVE: Provider<dyn Recorder> = Provider {
    name: "gsr-native",
    probe: |p| p.tools.has("gpu-screen-recorder").then_some(30),
    build: |p| {
        if test_mode(p) {
            return not_in_test_mode("recorder");
        }
        usable(es_record::GsrRecorder::native())
    },
};

pub const WF_RECORDER: Provider<dyn Recorder> = Provider {
    name: "wf-recorder",
    probe: |p| p.tools.has("wf-recorder").then_some(20),
    build: |p| {
        if test_mode(p) {
            return not_in_test_mode("recorder");
        }
        let encoders = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdin(std::process::Stdio::null())
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .map_err(|e| BackendError::Unavailable(format!("ffmpeg: {e}")))?;
        let recorder = es_record::WfRecorder::new(encoders, None);
        let recorder = match es_record::audio::default_sink_monitor(&es_record::audio::SystemRunner)
        {
            Some(monitor) => recorder.with_system_audio_device(monitor),
            None => recorder,
        };
        usable(recorder)
    },
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
