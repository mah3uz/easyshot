mod catalogue;
mod keys;
mod route;

pub use catalogue::{CATALOGUE, entry_by_id, entry_for_route};
pub use keys::{Modifiers, Trigger, key_label, key_recording_name, key_spoken_name};
pub use route::PATH_ARG;

use std::collections::BTreeMap;
use std::path::PathBuf;

pub const BUS_NAME: &str = "dev.easyshot.Easyshot";
pub const BUS_NAME_TEST: &str = "dev.easyshot.EasyshotTest";
pub const OBJECT_PATH: &str = "/dev/easyshot/Easyshot";
pub const INTERFACE: &str = "dev.easyshot.Easyshot";
/// Test-hooks-only interface: `Inject(events s)`, `SetPointer(x d, y d)`, `Dump() -> s`.
pub const TEST_INTERFACE: &str = "dev.easyshot.Test";

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    General,
    Capture,
    Overlay,
    AfterCapture,
    Recording,
    DefaultLook,
    Compositor,
    Editor,
    Ocr,
    Shortcuts,
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
