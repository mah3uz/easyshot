pub mod agent;
mod catalogue;
mod keys;
mod route;
mod timed;

pub use agent::AgentRoute;
pub use catalogue::{CATALOGUE, entry_by_id, entry_for_route};
pub use keys::{Modifiers, Trigger, key_label, key_recording_name, key_spoken_name};
pub use route::PATH_ARG;
pub use timed::{MAX_TIMED_SECONDS, SECONDS_ARG, TimedMode};

use std::collections::BTreeMap;
use std::path::PathBuf;

#[macro_export]
macro_rules! app_id {
    () => {
        "dev.easyshot.app"
    };
    ($part:literal) => {
        concat!($crate::app_id!(), ".", $part)
    };
}

pub const APP_ID: &str = app_id!();
pub const BUS_NAME: &str = app_id!();
pub const BUS_NAME_TEST: &str = app_id!("test");
pub const OBJECT_PATH: &str = "/dev/easyshot/app";
pub const INTERFACE: &str = app_id!();
/// Test-hooks-only interface: `Inject(events s)`, `SetPointer(x d, y d)`, `Dump() -> s`.
pub const TEST_INTERFACE: &str = app_id!("Test");
pub const EDITOR_APP_ID: &str = app_id!("editor");
pub const SETTINGS_APP_ID: &str = app_id!("settings");
pub const GALLERY_APP_ID: &str = app_id!("gallery");
pub const ONBOARDING_APP_ID: &str = app_id!("onboarding");
pub const PLAYER_APP_ID: &str = app_id!("player");
pub const ICON_NAME: &str = app_id!();

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("unknown route: {0}")]
    UnknownRoute(String),
    #[error("invalid arguments for route {route}: {reason}")]
    InvalidArgs { route: String, reason: String },
    #[error("url not allowed: {0}")]
    UrlNotAllowed(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    CaptureRegion,
    CaptureFullscreen,
    CaptureWindow,
    CapturePrevious,
    CaptureTimed,
    /// Built with `Route::timed`; `None` seconds is the self-timer setting.
    Timed {
        mode: TimedMode,
        seconds: Option<u32>,
    },
    CaptureRegionCopy,
    CaptureRegionSave,
    CaptureRegionAnnotate,
    CaptureRegionPin,
    CaptureScroll,
    Ocr,
    OcrSingleLine,
    ColorPicker,
    Record,
    RecordArea,
    RecordStop,
    RecordPause,
    RecordRestart,
    RecordDiscard,
    CaptureBar,
    Home,
    Gallery,
    RestoreLast,
    PinLast,
    UnpinAll,
    Open(PathBuf),
    EditClipboard,
    Settings(Option<SettingsPage>),
    DeckToggle,
    DeckSaveAll,
    DeckCloseAll,
    Quit,
    Agent(AgentRoute),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    Home,
    General,
    Capture,
    Overlay,
    Recording,
    Shortcuts,
    About,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    Str(String),
    Bool(bool),
    Int(i64),
    Path(PathBuf),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RouteArgs(pub BTreeMap<String, ArgValue>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Ok,
    Cancelled,
    Error,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            JobStatus::Ok => "ok",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Global,
    Editor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Route(Route),
    Action(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutEntry {
    pub id: &'static str,
    pub scope: Scope,
    pub title_id: &'static str,
    pub default_trigger: Option<&'static str>,
    pub target: Target,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_status_strings_match_the_dbus_signal() {
        assert_eq!(JobStatus::Ok.as_str(), "ok");
        assert_eq!(JobStatus::Cancelled.as_str(), "cancelled");
        assert_eq!(JobStatus::Error.as_str(), "error");
    }

    #[test]
    fn test_bus_is_distinct_from_the_real_one() {
        assert_ne!(BUS_NAME, BUS_NAME_TEST);
        assert_eq!(OBJECT_PATH, format!("/{}", BUS_NAME.replace('.', "/")));
    }
}
