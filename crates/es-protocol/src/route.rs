use std::borrow::Cow;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{ArgValue, Error, Result, Route, RouteArgs, SettingsPage};

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
    ("gallery", Route::Gallery),
    ("restore-last", Route::RestoreLast),
    ("pin-last", Route::PinLast),
    ("unpin-all", Route::UnpinAll),
    ("edit-clipboard", Route::EditClipboard),
    ("deck/toggle", Route::DeckToggle),
    ("deck/save-all", Route::DeckSaveAll),
    ("deck/close-all", Route::DeckCloseAll),
    ("quit", Route::Quit),
];

static URL_ROUTES: &[(&str, Route)] = &[
    ("capture/region", Route::CaptureRegion),
    ("capture/fullscreen", Route::CaptureFullscreen),
    ("capture/window", Route::CaptureWindow),
    ("capture/scroll", Route::CaptureScroll),
    ("record", Route::Record),
    ("ocr", Route::Ocr),
    ("color-picker", Route::ColorPicker),
    ("settings", Route::Settings(None)),
];

const URL_SCHEME: &str = "easyshot";
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];
const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G'];
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];

impl SettingsPage {
    pub const ALL: [SettingsPage; 10] = [
        SettingsPage::General,
        SettingsPage::Capture,
        SettingsPage::Overlay,
        SettingsPage::AfterCapture,
        SettingsPage::Recording,
        SettingsPage::DefaultLook,
        SettingsPage::Compositor,
        SettingsPage::Editor,
        SettingsPage::Ocr,
        SettingsPage::Shortcuts,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SettingsPage::General => "general",
            SettingsPage::Capture => "capture",
            SettingsPage::Overlay => "overlay",
            SettingsPage::AfterCapture => "after-capture",
            SettingsPage::Recording => "recording",
            SettingsPage::DefaultLook => "default-look",
            SettingsPage::Compositor => "compositor",
            SettingsPage::Editor => "editor",
            SettingsPage::Ocr => "ocr",
            SettingsPage::Shortcuts => "shortcuts",
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

    /// Accepts exactly eight routes, with no arguments.
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

    pub fn as_str(&self) -> Cow<'static, str> {
        match self {
            Route::Open(_) => Cow::Borrowed("open"),
            Route::Settings(None) => Cow::Borrowed("settings"),
            Route::Settings(Some(page)) => Cow::Owned(format!("settings/{}", page.as_str())),
            other => Cow::Borrowed(
                PLAIN
                    .iter()
                    .find(|(_, r)| r == other)
                    .map_or("", |(name, _)| name),
            ),
        }
    }

    pub fn args(&self) -> RouteArgs {
        let mut args = RouteArgs::default();
        if let Route::Open(path) = self {
            args.0
                .insert(PATH_ARG.to_owned(), ArgValue::Path(path.clone()));
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
        if !canonical.is_file() {
            return Err(bad("not a regular file"));
        }
        if !has_image_extension(&canonical) || !has_image_magic(&canonical) {
            return Err(bad("not a PNG or JPEG image"));
        }
        Ok(Route::Open(canonical))
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

fn has_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

fn has_image_magic(path: &Path) -> bool {
    let mut head = [0u8; 4];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let n = file.read(&mut head).unwrap_or(0);
    head[..n].starts_with(PNG_MAGIC) || head[..n].starts_with(JPEG_MAGIC)
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

    #[test]
    fn every_plain_route_round_trips_through_its_name() {
        for (name, route) in PLAIN {
            assert_eq!(route.as_str(), *name);
            assert_eq!(route.to_string(), *name);
            assert_eq!(&Route::parse(name, &no_args()).unwrap(), route);
        }
        assert_eq!(PLAIN.len(), 29);
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
    fn url_accepts_exactly_the_eight_routes() {
        for (name, route) in URL_ROUTES {
            assert_eq!(
                &Route::from_url(&format!("easyshot://{name}")).unwrap(),
                route
            );
        }
        assert_eq!(URL_ROUTES.len(), 8);
        assert_eq!(
            Route::from_url("easyshot://settings").unwrap(),
            Route::Settings(None)
        );
    }

    #[test]
    fn url_scheme_and_host_ignore_case_but_the_path_does_not() {
        assert_eq!(
            Route::from_url("EASYSHOT://CAPTURE/region").unwrap(),
            Route::CaptureRegion
        );
        assert_eq!(Route::from_url("EasyShot://RECORD").unwrap(), Route::Record);
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
