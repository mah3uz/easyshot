mod apps;
mod autostart;
mod bus;
pub mod dbus;
mod files;
mod login;
mod noop;
mod notify;
mod portal;
mod secrets;
mod sound;
mod tray;
mod tray_icon;

pub use apps::{AppLabel, app_label, app_label_in, app_name, app_name_in, fallback_name};
pub use autostart::{autostart_enabled, autostart_file, set_autostart, set_autostart_in};
pub use bus::Labels;
pub use files::{
    move_to_trash, restore_from_trash, show_in_file_manager, show_items, spawn_reaped,
};
pub use login::{follow_login_session, watch_login_session};
pub use noop::{RecordingNotifier, RecordingShortcuts, RecordingSound, RecordingTray};
pub use notify::{DEFAULT_ACTION, DesktopNotifier};
pub use portal::{PortalAppearance, PortalFilePicker, PortalShortcuts, portal_trigger};
pub use secrets::{
    SecretsError, available, clear_bucket_keys, load_bucket_keys, store_bucket_keys,
};
#[cfg(feature = "sound")]
pub use sound::RodioSound;
pub use sound::Sound;
pub use tray::{IconPixmap, KsniTray, host_within, watcher_present};
pub use tray_icon::{tray_icons, with_badge, with_dot};

use es_protocol::{Route, ShortcutEntry};
use futures::future::BoxFuture;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unsupported on this system")]
    Unsupported,
    #[error("cancelled")]
    Cancelled,
    #[error("timed out")]
    Timeout,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("parse error at line {line}, column {col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Shortcuts: Send + Sync {
    fn register(&self, defs: &[ShortcutEntry]) -> Result<()>;
    fn activations(&self) -> async_channel::Receiver<Route>;
    fn user_binds_required(&self) -> bool;
}

pub trait Tray: Send + Sync {
    fn set_menu(&self, m: TrayModel);
    fn set_recording(&self, s: Option<RecordingTrayState>);
    fn set_agent(&self, active: bool);
    fn activations(&self) -> async_channel::Receiver<Route>;
}

#[derive(Debug, Clone, Default)]
pub struct TrayModel {
    pub items: Vec<TrayItem>,
}

#[derive(Debug, Clone)]
pub struct TrayItem {
    pub label_id: &'static str,
    /// Shown instead of looking up `label_id`, for text that carries a value.
    pub label: Option<String>,
    pub shortcut: Option<String>,
    pub route: Option<Route>,
    pub children: Vec<TrayItem>,
    pub enabled: bool,
    pub separator: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingTrayState {
    pub elapsed: Duration,
    pub paused: bool,
}

pub trait Notifier: Send + Sync {
    fn notify(&self, n: Notification) -> Result<NotificationId>;
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)>;
    fn supports_actions(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub icon: Option<PathBuf>,
    pub image: Option<PathBuf>,
    /// `(id, label)` pairs.
    pub actions: Vec<(String, String)>,
    pub urgency: Urgency,
    pub timeout: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub title: String,
    pub suggested_name: String,
    pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct OpenRequest {
    pub title: String,
    pub multiple: bool,
    pub dir: Option<PathBuf>,
    pub filter: Option<OpenFilter>,
}

#[derive(Debug, Clone)]
pub struct OpenFilter {
    pub label: String,
    pub mime_types: Vec<String>,
}

pub trait FilePicker: Send + Sync {
    fn save(&self, req: SaveRequest) -> BoxFuture<'static, Result<Option<PathBuf>>>;
    fn open(&self, req: OpenRequest) -> BoxFuture<'static, Result<Vec<PathBuf>>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceEvent {
    ColorScheme(ColorScheme),
    ReduceMotion(bool),
}

pub trait Appearance: Send + Sync {
    fn color_scheme(&self) -> ColorScheme;
    fn reduce_motion(&self) -> bool;
    fn changes(&self) -> async_channel::Receiver<AppearanceEvent>;
}
