use std::borrow::Cow;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::agent::{self, AgentRoute};
use crate::timed::TimedMode;
use crate::{ArgValue, Error, Result, Route, RouteArgs, SettingsPage, timed};

pub const PATH_ARG: &str = "path";

static PLAIN: &[(&str, Route)] = &[
    ("capture/region", Route::CaptureRegion),
    ("capture/fullscreen", Route::CaptureFullscreen),
    ("capture/window", Route::CaptureWindow),
    ("capture/previous", Route::CapturePrevious),
    ("capture/timed", Route::CaptureTimed),
    ("capture/region-copy", Route::CaptureRegionCopy),
    ("capture/region-save", Route::CaptureRegionSave),
    ("capture/region-annotate", Route::CaptureRegionAnnotate),
    ("capture/region-pin", Route::CaptureRegionPin),
    ("capture/scroll", Route::CaptureScroll),
    ("capture/scroll-window", Route::CaptureScrollWindow),
    ("ocr", Route::Ocr),
    ("ocr/single-line", Route::OcrSingleLine),
    ("color-picker", Route::ColorPicker),
    ("record", Route::Record),
    ("record/area", Route::RecordArea),
    ("record/stop", Route::RecordStop),
    ("record/pause", Route::RecordPause),
    ("record/restart", Route::RecordRestart),
    ("record/discard", Route::RecordDiscard),
    ("capture-bar", Route::CaptureBar),
    ("home", Route::Home),
    ("gallery", Route::Gallery),
    ("restore-last", Route::RestoreLast),
    ("pin-last", Route::PinLast),
    ("unpin-all", Route::UnpinAll),
    ("edit-clipboard", Route::EditClipboard),
    ("deck/toggle", Route::DeckToggle),
    ("deck/save-all", Route::DeckSaveAll),
    ("deck/close-all", Route::DeckCloseAll),
    ("quit", Route::Quit),
    ("agent-revoke", Route::AgentRevoke),
];

static URL_ROUTES: &[(&str, Route)] = &[
    ("capture/region", Route::CaptureRegion),
    ("capture/fullscreen", Route::CaptureFullscreen),
    ("capture/window", Route::CaptureWindow),
    ("capture/scroll", Route::CaptureScroll),
    ("record", Route::RecordArea),
    ("ocr", Route::Ocr),
    ("color-picker", Route::ColorPicker),
    ("home", Route::Home),
    ("settings", Route::Settings(None)),
];

const URL_SCHEME: &str = "easyshot";
const OPEN_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "mp4", "webm", "mkv", "mov", "gif"];
const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G'];
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];
const GIF_MAGIC: &[u8] = b"GIF8";
const MATROSKA_MAGIC: &[u8] = &[0x1A, 0x45, 0xDF, 0xA3];
const ISO_BASE_BRAND_AT: usize = 4;

impl SettingsPage {
    pub const ALL: [SettingsPage; 8] = [
        SettingsPage::Home,
        SettingsPage::General,
        SettingsPage::Capture,
        SettingsPage::Overlay,
        SettingsPage::Recording,
        SettingsPage::Shortcuts,
        SettingsPage::Agents,
        SettingsPage::About,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SettingsPage::Home => "home",
            SettingsPage::General => "general",
            SettingsPage::Capture => "capture",
            SettingsPage::Overlay => "overlay",
            SettingsPage::Recording => "recording",
            SettingsPage::Shortcuts => "shortcuts",
            SettingsPage::Agents => "agents",
            SettingsPage::About => "about",
        }
    }

    pub fn from_name(name: &str) -> Option<SettingsPage> {
        SettingsPage::ALL.into_iter().find(|p| p.as_str() == name)
    }
}

impl Route {
    pub fn parse(route: &str, args: &RouteArgs) -> Result<Route> {
        let unknown = || Error::UnknownRoute(route.to_owned());
        if route == "open" {
            return Route::open_from_args(args);
        }
        if let Some(mode) = TimedMode::from_route_name(route) {
            return Route::timed_from_args(route, mode, args);
        }
        if route.starts_with(agent::PREFIX) {
            return AgentRoute::parse(route, args).map(Route::Agent);
        }
        let parsed = if let Some((_, r)) = PLAIN.iter().find(|(name, _)| *name == route) {
            r.clone()
        } else if route == "settings" {
            Route::Settings(None)
        } else if let Some(page) = route.strip_prefix("settings/") {
            Route::Settings(Some(SettingsPage::from_name(page).ok_or_else(unknown)?))
        } else {
            return Err(unknown());
        };
        if !args.0.is_empty() {
            return Err(Error::InvalidArgs {
                route: route.to_owned(),
                reason: "this route takes no arguments".to_owned(),
            });
        }
        Ok(parsed)
    }

    pub fn url_routes() -> impl Iterator<Item = String> {
        URL_ROUTES
            .iter()
            .map(|(name, _)| format!("{URL_SCHEME}://{name}"))
    }

    /// Accepts exactly nine routes, with no arguments.
    pub fn from_url(url: &str) -> Result<Route> {
        let denied = || Error::UrlNotAllowed(url.to_owned());
        let (scheme, rest) = url.split_once("://").ok_or_else(denied)?;
        if !scheme.eq_ignore_ascii_case(URL_SCHEME) || rest.contains(['?', '#']) {
            return Err(denied());
        }
        let (authority, path) = match rest.find('/') {
            Some(i) => rest.split_at(i),
            None => (rest, ""),
        };
        if authority.is_empty() || authority.contains(['@', ':']) {
            return Err(denied());
        }
        // Only the host is case-folded; a path in capitals is not a route.
        let key = format!("{}{}", authority.to_ascii_lowercase(), path);
        URL_ROUTES
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, r)| r.clone())
            .ok_or_else(denied)
    }

    /// One of every route; the variants that carry a value appear once per kind of value.
    pub fn all() -> Vec<Route> {
        let mut all: Vec<Route> = PLAIN.iter().map(|(_, r)| r.clone()).collect();
        all.push(Route::Open(PathBuf::from("image.png")));
        all.extend(TimedMode::ALL.map(|mode| Route::timed(mode, Some(3))));
        all.push(Route::Settings(None));
        all.extend(SettingsPage::ALL.map(|p| Route::Settings(Some(p))));
        all.extend(AgentRoute::ALL.map(Route::Agent));
        all
    }

    pub fn as_str(&self) -> Cow<'static, str> {
        match self {
            Route::Open(_) => Cow::Borrowed("open"),
            Route::Timed { mode, .. } => Cow::Borrowed(mode.route_name()),
            Route::Settings(None) => Cow::Borrowed("settings"),
            Route::Settings(Some(page)) => Cow::Owned(format!("settings/{}", page.as_str())),
            Route::Agent(a) => Cow::Borrowed(a.route_name()),
            other => Cow::Borrowed(
                PLAIN
                    .iter()
                    .find(|(_, r)| r == other)
                    .map_or("", |(name, _)| name),
            ),
        }
    }

    /// The words after `easyshot` on a command line that runs this route.
    pub fn command_args(&self) -> Vec<&'static str> {
        match self {
            Route::CaptureRegionCopy => vec!["capture", "region", "--copy"],
            Route::CaptureRegionSave => vec!["capture", "region", "--save"],
            Route::CaptureRegionAnnotate => vec!["capture", "region", "--annotate"],
            Route::CaptureRegionPin => vec!["capture", "region", "--pin"],
            Route::CaptureScrollWindow => vec!["capture", "scroll", "--window"],
            Route::OcrSingleLine => vec!["ocr", "--single-line"],
            Route::Settings(Some(page)) => vec!["settings", page.as_str()],
            Route::Settings(None) => vec!["settings"],
            Route::Home => vec![],
            Route::Open(_) => vec!["open"],
            Route::Timed { mode, .. } => vec!["capture", mode.word(), "--timed"],
            Route::Agent(a) => a.command_args(),
            other => PLAIN
                .iter()
                .find(|(_, r)| r == other)
                .map_or("", |(name, _)| name)
                .split('/')
                .collect(),
        }
    }

    pub fn args(&self) -> RouteArgs {
        let mut args = RouteArgs::default();
        match self {
            Route::Agent(a) => return a.args(),
            Route::Open(path) => {
                args.0
                    .insert(PATH_ARG.to_owned(), ArgValue::Path(path.clone()));
            }
            Route::Timed {
                seconds: Some(n), ..
            } => {
                args.0
                    .insert(timed::SECONDS_ARG.to_owned(), ArgValue::Int(i64::from(*n)));
            }
            _ => {}
        }
        args
    }

    fn open_from_args(args: &RouteArgs) -> Result<Route> {
        let bad = |reason: &str| Error::InvalidArgs {
            route: "open".to_owned(),
            reason: reason.to_owned(),
        };
        if args.0.len() != 1 {
            return Err(bad("expected exactly one argument: path"));
        }
        let path = match args.0.get(PATH_ARG) {
            Some(ArgValue::Path(p)) => p.clone(),
            Some(ArgValue::Str(s)) => PathBuf::from(s),
            _ => return Err(bad("path must be a string or path")),
        };
        if path.as_os_str().is_empty() {
            return Err(bad("path is empty"));
        }
        let canonical = path.canonicalize().map_err(|e| bad(&e.to_string()))?;
        if !has_openable_extension(&canonical) {
            return Err(bad("not a PNG or JPEG image or a video"));
        }
        let mut file = open_regular(&canonical).ok_or_else(|| bad("not a regular file"))?;
        if !has_openable_magic(&mut file) {
            return Err(bad("not a PNG or JPEG image or a video"));
        }
        Ok(Route::Open(canonical))
    }
}

/// Non-blocking and no-follow, so a FIFO or link swapped in after `canonicalize` cannot hang us.
fn open_regular(path: &Path) -> Option<std::fs::File> {
    use rustix::fs::{FileType, Mode, OFlags};
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::open(path, flags, Mode::empty()).ok()?;
    let stat = rustix::fs::fstat(&fd).ok()?;
    (FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile).then(|| fd.into())
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

fn has_openable_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OPEN_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

fn has_openable_magic(file: &mut std::fs::File) -> bool {
    let mut head = [0u8; 8];
    let n = file.read(&mut head).unwrap_or(0);
    let head = &head[..n];
    head.starts_with(PNG_MAGIC)
        || head.starts_with(JPEG_MAGIC)
        || head.starts_with(GIF_MAGIC)
        || head.starts_with(MATROSKA_MAGIC)
        || head
            .get(ISO_BASE_BRAND_AT..)
            .is_some_and(|b| b.starts_with(b"ftyp"))
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
