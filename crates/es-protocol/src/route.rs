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
    ("capture/scroll-region", Route::CaptureScrollRegion),
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
            Route::CaptureScrollRegion => vec!["capture", "scroll", "--region"],
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
mod tests {
    use super::*;

    fn no_args() -> RouteArgs {
        RouteArgs::default()
    }

    fn path_args(p: &Path) -> RouteArgs {
        let mut a = RouteArgs::default();
        a.0.insert(PATH_ARG.into(), ArgValue::Path(p.to_owned()));
        a
    }

    fn variant_index(route: &Route) -> usize {
        match route {
            Route::CaptureRegion => 0,
            Route::CaptureFullscreen => 1,
            Route::CaptureWindow => 2,
            Route::CapturePrevious => 3,
            Route::CaptureTimed => 4,
            Route::Timed { .. } => 5,
            Route::CaptureRegionCopy => 6,
            Route::CaptureRegionSave => 7,
            Route::CaptureRegionAnnotate => 8,
            Route::CaptureRegionPin => 9,
            Route::CaptureScroll => 10,
            Route::CaptureScrollRegion => 35,
            Route::Ocr => 11,
            Route::OcrSingleLine => 12,
            Route::ColorPicker => 13,
            Route::Record => 14,
            Route::RecordArea => 15,
            Route::RecordStop => 16,
            Route::RecordPause => 17,
            Route::RecordRestart => 18,
            Route::RecordDiscard => 19,
            Route::CaptureBar => 20,
            Route::Home => 21,
            Route::Gallery => 22,
            Route::RestoreLast => 23,
            Route::PinLast => 24,
            Route::UnpinAll => 25,
            Route::Open(_) => 26,
            Route::EditClipboard => 27,
            Route::Settings(_) => 28,
            Route::DeckToggle => 29,
            Route::DeckSaveAll => 30,
            Route::DeckCloseAll => 31,
            Route::Quit => 32,
            Route::AgentRevoke => 33,
            Route::Agent(_) => 34,
        }
    }

    #[test]
    fn all_lists_every_variant() {
        let seen: std::collections::BTreeSet<usize> =
            Route::all().iter().map(variant_index).collect();
        assert_eq!(
            seen,
            (0..=35).collect(),
            "a variant is missing from Route::all"
        );
    }

    #[test]
    fn every_plain_route_round_trips_through_its_name() {
        for (name, route) in PLAIN {
            assert_eq!(route.as_str(), *name);
            assert_eq!(route.to_string(), *name);
            assert_eq!(&Route::parse(name, &no_args()).unwrap(), route);
        }
        assert_eq!(PLAIN.len(), 32);
    }

    #[test]
    fn settings_pages_round_trip() {
        let bare = Route::parse("settings", &no_args()).unwrap();
        assert_eq!(bare, Route::Settings(None));
        for page in SettingsPage::ALL {
            let r = Route::Settings(Some(page));
            assert_eq!(Route::parse(&r.as_str(), &no_args()).unwrap(), r);
        }
        assert!(Route::parse("settings/nope", &no_args()).is_err());
        assert!(Route::parse("settings/", &no_args()).is_err());
        for old in [
            "after-capture",
            "default-look",
            "compositor",
            "editor",
            "ocr",
        ] {
            assert!(
                Route::parse(&format!("settings/{old}"), &no_args()).is_err(),
                "{old}"
            );
        }
    }

    #[test]
    fn unknown_routes_are_rejected() {
        for r in [
            "",
            "Capture/Region",
            "capture",
            "capture/region/",
            "/ocr",
            "exec",
            "quit ",
        ] {
            assert!(
                matches!(Route::parse(r, &no_args()), Err(Error::UnknownRoute(_))),
                "{r:?}"
            );
        }
    }

    #[test]
    fn routes_other_than_open_take_no_arguments() {
        let mut args = RouteArgs::default();
        args.0
            .insert("output".into(), ArgValue::Str("/tmp/x.png".into()));
        for route in [
            "capture/region",
            "record",
            "settings",
            "settings/general",
            "quit",
        ] {
            assert!(
                matches!(Route::parse(route, &args), Err(Error::InvalidArgs { .. })),
                "{route}"
            );
        }
    }

    #[test]
    fn open_requires_an_existing_image_and_canonicalises() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.PNG");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
        let jpg = dir.path().join("b.jpg");
        std::fs::write(&jpg, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        let fake = dir.path().join("c.png");
        std::fs::write(&fake, b"hello").unwrap();
        let text = dir.path().join("d.txt");
        std::fs::write(&text, [0x89, b'P', b'N', b'G']).unwrap();
        let mismatch = dir.path().join("e.png");
        std::fs::write(&mismatch, [0xFF, 0xD8, 0xFF]).unwrap();

        let roundabout = dir.path().join(".").join("a.PNG");
        let r = Route::parse("open", &path_args(&roundabout)).unwrap();
        assert_eq!(r, Route::Open(png.canonicalize().unwrap()));
        assert!(Route::parse("open", &path_args(&jpg)).is_ok());
        for bad in [&fake, &text, &dir.path().join("missing.png"), dir.path()] {
            assert!(
                matches!(
                    Route::parse("open", &path_args(bad)),
                    Err(Error::InvalidArgs { .. })
                ),
                "{bad:?}"
            );
        }
        assert!(Route::parse("open", &path_args(&mismatch)).is_ok());
    }

    #[test]
    fn open_accepts_videos_by_extension_and_container() {
        let dir = tempfile::tempdir().unwrap();
        let heads: [(&str, &[u8]); 5] = [
            ("a.mp4", b"\0\0\0\x18ftypmp42"),
            ("b.MOV", b"\0\0\0\x14ftypqt  "),
            ("c.webm", &[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
            ("d.mkv", &[0x1A, 0x45, 0xDF, 0xA3, 0x9F]),
            ("e.gif", b"GIF89a"),
        ];
        for (name, head) in heads {
            let file = dir.path().join(name);
            std::fs::write(&file, head).unwrap();
            let r = Route::parse("open", &path_args(&file)).unwrap();
            assert_eq!(r, Route::Open(file.canonicalize().unwrap()), "{name}");
        }
        let fake = dir.path().join("f.mp4");
        std::fs::write(&fake, b"not a video at all").unwrap();
        assert!(Route::parse("open", &path_args(&fake)).is_err());
    }

    #[test]
    fn open_refuses_a_fifo_without_blocking() {
        use rustix::fs::{CWD, FileType, Mode, mknodat};
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("trap.png");
        mknodat(CWD, &fifo, FileType::Fifo, Mode::RUSR | Mode::WUSR, 0).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let args = path_args(&fifo);
        std::thread::spawn(move || {
            let _ = tx.send(Route::parse("open", &args));
        });
        let parsed = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("parsing a FIFO must not block");
        assert!(matches!(parsed, Err(Error::InvalidArgs { .. })));
    }

    #[test]
    fn a_link_never_records_without_a_selection() {
        for (_, route) in URL_ROUTES {
            assert_ne!(route, &Route::Record);
        }
    }

    #[test]
    fn open_rejects_missing_wrong_or_extra_arguments() {
        assert!(Route::parse("open", &no_args()).is_err());
        let mut args = RouteArgs::default();
        args.0.insert(PATH_ARG.into(), ArgValue::Bool(true));
        assert!(Route::parse("open", &args).is_err());
        let mut args = RouteArgs::default();
        args.0.insert("other".into(), ArgValue::Str("x".into()));
        assert!(Route::parse("open", &args).is_err());
    }

    #[test]
    fn open_round_trips_through_name_and_args() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G']).unwrap();
        let r = Route::parse("open", &path_args(&png)).unwrap();
        assert_eq!(r.as_str(), "open");
        assert_eq!(Route::parse(&r.as_str(), &r.args()).unwrap(), r);
    }

    #[test]
    fn url_routes_lists_every_allowed_url() {
        let urls: Vec<_> = Route::url_routes().collect();
        assert_eq!(urls.len(), URL_ROUTES.len());
        for url in urls {
            assert!(Route::from_url(&url).is_ok(), "{url}");
        }
    }

    #[test]
    fn url_accepts_exactly_the_nine_routes() {
        for (name, route) in URL_ROUTES {
            assert_eq!(
                &Route::from_url(&format!("easyshot://{name}")).unwrap(),
                route
            );
        }
        assert_eq!(URL_ROUTES.len(), 9);
        assert_eq!(
            Route::from_url("easyshot://settings").unwrap(),
            Route::Settings(None)
        );
        assert_eq!(Route::from_url("easyshot://home").unwrap(), Route::Home);
    }

    #[test]
    fn url_scheme_and_host_ignore_case_but_the_path_does_not() {
        assert_eq!(
            Route::from_url("EASYSHOT://CAPTURE/region").unwrap(),
            Route::CaptureRegion
        );
        assert_eq!(
            Route::from_url("EasyShot://RECORD").unwrap(),
            Route::RecordArea
        );
        assert!(Route::from_url("easyshot://capture/REGION").is_err());
    }

    #[test]
    fn url_rejects_everything_outside_the_allowlist() {
        for url in [
            "https://capture/region",
            "easyshot:record",
            "easyshot:///record",
            "easyshot://",
            "easyshot://unknown",
            "easyshot://capture",
            "easyshot://capture/region/extra",
            "easyshot://capture/region/",
            "easyshot://record/",
            "easyshot://capture/region?output=/tmp/test.png",
            "easyshot://record?",
            "easyshot://record#start",
            "easyshot://record#",
            "easyshot://user@record",
            "easyshot://user:password@record",
            "easyshot://record:123",
            "easyshot://capture/../region",
            "easyshot://settings/anything",
            "easyshot://settings/general",
            "easyshot://home/",
            "easyshot://home/gallery",
            "easyshot://home?tab=settings",
            "easyshot://gallery",
            "easyshot://capture/previous",
            "easyshot://quit",
            "easyshot://open",
            " easyshot://record",
            "easyshot://record ",
            "",
        ] {
            assert!(
                matches!(Route::from_url(url), Err(Error::UrlNotAllowed(_))),
                "{url:?}"
            );
        }
    }
}
