use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::engine::ArgValueCompleter;
use es_protocol::{ArgValue, PATH_ARG, Route, RouteArgs, SettingsPage, TimedMode};

#[derive(Parser, Debug)]
#[command(
    name = "easyshot",
    version,
    about = "Screenshot, annotation and screen recording"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Take a screenshot
    Capture(CaptureArgs),
    /// Recognise text in a region
    Ocr {
        /// Read one line and copy it without line breaks
        #[arg(long)]
        single_line: bool,
    },
    /// Pick a colour from the screen
    ColorPicker,
    /// Record the screen
    Record { action: Option<RecordAction> },
    /// Show the capture bar
    CaptureBar,
    /// Open the gallery
    Gallery,
    /// Open settings, optionally on one page
    Settings {
        #[arg(value_parser = PageParser)]
        page: Option<SettingsPage>,
    },
    /// Open an image file in the editor, or handle an easyshot:// URL
    Open {
        #[arg(add = ArgValueCompleter::new(crate::complete::open_target))]
        target: String,
    },
    /// Edit the image on the clipboard
    EditClipboard,
    /// Act on the deck
    Deck { action: DeckAction },
    /// Reopen the last capture
    RestoreLast,
    /// Pin the last capture
    PinLast,
    /// Close every pin
    UnpinAll,
    /// Quit the running daemon
    Quit,
    /// Report what this machine supports and which providers were chosen
    Doctor {
        /// Machine-readable output
        #[arg(long)]
        json: bool,
        /// Write a tarball of the report and the logs
        #[arg(long)]
        bundle: bool,
    },
    /// Run the daemon
    Daemon,
    /// Print a shell completion script
    Completions { shell: clap_complete::Shell },
}

#[derive(Args, Debug)]
pub struct CaptureArgs {
    pub mode: CaptureMode,
    /// Copy the region to the clipboard
    #[arg(long, conflicts_with_all = ["save", "annotate", "pin"])]
    pub copy: bool,
    /// Save the region to the library
    #[arg(long, conflicts_with_all = ["annotate", "pin"])]
    pub save: bool,
    /// Open the region in the editor
    #[arg(long, conflicts_with = "pin")]
    pub annotate: bool,
    /// Pin the region on screen
    #[arg(long)]
    pub pin: bool,
    /// Count down N seconds after you pick, then capture; without N, the self-timer setting
    #[arg(
        long,
        value_name = "N",
        num_args = 0..=1,
        value_parser = clap::value_parser!(u32).range(1..=i64::from(es_protocol::MAX_TIMED_SECONDS)),
        conflicts_with_all = ["copy", "save", "annotate", "pin"]
    )]
    pub timed: Option<Option<u32>>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureMode {
    Region,
    Fullscreen,
    Window,
    Previous,
    Timed,
    Scroll,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordAction {
    Area,
    Stop,
    Pause,
    Restart,
    Discard,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckAction {
    Toggle,
    SaveAll,
    CloseAll,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Route(Route),
    Doctor { json: bool, bundle: bool },
    Daemon,
    Completions(clap_complete::Shell),
}

#[derive(Clone)]
struct PageParser;

impl clap::builder::TypedValueParser for PageParser {
    type Value = SettingsPage;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> std::result::Result<SettingsPage, clap::Error> {
        value
            .to_str()
            .and_then(SettingsPage::from_name)
            .ok_or_else(|| {
                let known: Vec<_> = SettingsPage::ALL.iter().map(|p| p.as_str()).collect();
                clap::Error::raw(
                    clap::error::ErrorKind::InvalidValue,
                    format!(
                        "unknown settings page, expected one of: {}\n",
                        known.join(", ")
                    ),
                )
                .with_cmd(cmd)
            })
    }

    fn possible_values(
        &self,
    ) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
        Some(Box::new(
            SettingsPage::ALL
                .iter()
                .map(|p| clap::builder::PossibleValue::new(p.as_str())),
        ))
    }
}

impl Cli {
    pub fn request(&self) -> Result<Request> {
        Ok(match &self.command {
            Command::Capture(a) => Request::Route(capture_route(a)?),
            Command::Ocr { single_line } => Request::Route(if *single_line {
                Route::OcrSingleLine
            } else {
                Route::Ocr
            }),
            Command::ColorPicker => Request::Route(Route::ColorPicker),
            Command::Record { action } => Request::Route(match action {
                None => Route::Record,
                Some(RecordAction::Area) => Route::RecordArea,
                Some(RecordAction::Stop) => Route::RecordStop,
                Some(RecordAction::Pause) => Route::RecordPause,
                Some(RecordAction::Restart) => Route::RecordRestart,
                Some(RecordAction::Discard) => Route::RecordDiscard,
            }),
            Command::CaptureBar => Request::Route(Route::CaptureBar),
            Command::Gallery => Request::Route(Route::Gallery),
            Command::Settings { page } => Request::Route(Route::Settings(*page)),
            Command::Open { target } => Request::Route(open_route(target)?),
            Command::EditClipboard => Request::Route(Route::EditClipboard),
            Command::Deck { action } => Request::Route(match action {
                DeckAction::Toggle => Route::DeckToggle,
                DeckAction::SaveAll => Route::DeckSaveAll,
                DeckAction::CloseAll => Route::DeckCloseAll,
            }),
            Command::RestoreLast => Request::Route(Route::RestoreLast),
            Command::PinLast => Request::Route(Route::PinLast),
            Command::UnpinAll => Request::Route(Route::UnpinAll),
            Command::Quit => Request::Route(Route::Quit),
            Command::Doctor { json, bundle } => Request::Doctor {
                json: *json,
                bundle: *bundle,
            },
            Command::Daemon => Request::Daemon,
            Command::Completions { shell } => Request::Completions(*shell),
        })
    }
}

fn capture_route(a: &CaptureArgs) -> Result<Route> {
    if let Some(seconds) = a.timed {
        let mode = match a.mode {
            CaptureMode::Region | CaptureMode::Timed => TimedMode::Region,
            CaptureMode::Window => TimedMode::Window,
            CaptureMode::Fullscreen => TimedMode::Fullscreen,
            CaptureMode::Previous | CaptureMode::Scroll => {
                bail!("--timed applies to `capture region`, `window` and `fullscreen` only")
            }
        };
        return Ok(Route::timed(mode, seconds));
    }
    let variant = match (a.copy, a.save, a.annotate, a.pin) {
        (false, false, false, false) => None,
        (true, ..) => Some(Route::CaptureRegionCopy),
        (_, true, ..) => Some(Route::CaptureRegionSave),
        (_, _, true, _) => Some(Route::CaptureRegionAnnotate),
        _ => Some(Route::CaptureRegionPin),
    };
    match (a.mode, variant) {
        (CaptureMode::Region, Some(r)) => Ok(r),
        (_, Some(_)) => {
            bail!("--copy, --save, --annotate and --pin apply to `capture region` only")
        }
        (CaptureMode::Region, None) => Ok(Route::CaptureRegion),
        (CaptureMode::Fullscreen, None) => Ok(Route::CaptureFullscreen),
        (CaptureMode::Window, None) => Ok(Route::CaptureWindow),
        (CaptureMode::Previous, None) => Ok(Route::CapturePrevious),
        (CaptureMode::Timed, None) => Ok(Route::CaptureTimed),
        (CaptureMode::Scroll, None) => Ok(Route::CaptureScroll),
    }
}

fn open_route(target: &str) -> Result<Route> {
    if target.contains("://") {
        return Route::from_url(target).map_err(|e| anyhow!("{e}"));
    }
    let mut args = RouteArgs::default();
    args.0
        .insert(PATH_ARG.to_owned(), ArgValue::Str(target.to_owned()));
    Route::parse("open", &args).map_err(|e| anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(args: &[&str]) -> Result<Request> {
        let cli = Cli::try_parse_from(std::iter::once("easyshot").chain(args.iter().copied()))
            .map_err(|e| anyhow!("{e}"))?;
        cli.request()
    }

    fn route(args: &[&str]) -> Route {
        match request(args).unwrap_or_else(|e| panic!("{args:?}: {e}")) {
            Request::Route(r) => r,
            other => panic!("not a route: {other:?}"),
        }
    }

    #[test]
    fn every_catalogue_snippet_command_parses_back_to_its_route() {
        for entry in es_protocol::CATALOGUE {
            let es_protocol::Target::Route(expected) = &entry.target else {
                continue;
            };
            if matches!(expected, Route::Open(_)) {
                continue;
            }
            let args = expected.command_args();
            assert_eq!(&route(&args), expected, "{}: {args:?}", entry.id);
        }
    }

    #[test]
    fn every_subcommand_maps_to_its_route() {
        let table: &[(&[&str], Route)] = &[
            (&["capture", "region"], Route::CaptureRegion),
            (&["capture", "fullscreen"], Route::CaptureFullscreen),
            (&["capture", "window"], Route::CaptureWindow),
            (&["capture", "previous"], Route::CapturePrevious),
            (&["capture", "timed"], Route::CaptureTimed),
            (&["capture", "region", "--timed"], Route::CaptureTimed),
            (
                &["capture", "region", "--timed", "5"],
                Route::timed(TimedMode::Region, Some(5)),
            ),
            (
                &["capture", "window", "--timed"],
                Route::timed(TimedMode::Window, None),
            ),
            (
                &["capture", "fullscreen", "--timed", "10"],
                Route::timed(TimedMode::Fullscreen, Some(10)),
            ),
            (
                &["capture", "timed", "--timed", "3"],
                Route::timed(TimedMode::Region, Some(3)),
            ),
            (&["capture", "scroll"], Route::CaptureScroll),
            (&["capture", "region", "--copy"], Route::CaptureRegionCopy),
            (&["capture", "region", "--save"], Route::CaptureRegionSave),
            (
                &["capture", "region", "--annotate"],
                Route::CaptureRegionAnnotate,
            ),
            (&["capture", "region", "--pin"], Route::CaptureRegionPin),
            (&["ocr"], Route::Ocr),
            (&["ocr", "--single-line"], Route::OcrSingleLine),
            (&["color-picker"], Route::ColorPicker),
            (&["record"], Route::Record),
            (&["record", "area"], Route::RecordArea),
            (&["record", "stop"], Route::RecordStop),
            (&["record", "pause"], Route::RecordPause),
            (&["record", "restart"], Route::RecordRestart),
            (&["record", "discard"], Route::RecordDiscard),
            (&["capture-bar"], Route::CaptureBar),
            (&["gallery"], Route::Gallery),
            (&["settings"], Route::Settings(None)),
            (
                &["settings", "shortcuts"],
                Route::Settings(Some(SettingsPage::Shortcuts)),
            ),
            (&["edit-clipboard"], Route::EditClipboard),
            (&["deck", "toggle"], Route::DeckToggle),
            (&["deck", "save-all"], Route::DeckSaveAll),
            (&["deck", "close-all"], Route::DeckCloseAll),
            (&["restore-last"], Route::RestoreLast),
            (&["pin-last"], Route::PinLast),
            (&["unpin-all"], Route::UnpinAll),
            (&["quit"], Route::Quit),
        ];
        for (args, expected) in table {
            assert_eq!(&route(args), expected, "{args:?}");
        }
    }

    #[test]
    fn every_settings_page_is_reachable() {
        for page in SettingsPage::ALL {
            assert_eq!(
                route(&["settings", page.as_str()]),
                Route::Settings(Some(page))
            );
        }
        assert!(request(&["settings", "nope"]).is_err());
    }

    #[test]
    fn action_flags_belong_to_region_and_exclude_each_other() {
        assert!(request(&["capture", "fullscreen", "--copy"]).is_err());
        assert!(request(&["capture", "region", "--copy", "--pin"]).is_err());
        assert!(request(&["capture", "region", "--save", "--annotate"]).is_err());
        assert!(request(&["capture", "bogus"]).is_err());
        assert!(request(&["record", "bogus"]).is_err());
    }

    #[test]
    fn timed_belongs_to_region_window_and_fullscreen_with_one_to_sixty_seconds() {
        for bad in [
            &["capture", "previous", "--timed"][..],
            &["capture", "scroll", "--timed", "3"],
            &["capture", "region", "--timed", "0"],
            &["capture", "window", "--timed", "61"],
            &["capture", "region", "--timed", "3", "--copy"],
        ] {
            assert!(request(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn open_takes_an_image_or_an_allowed_url() {
        assert_eq!(route(&["open", "easyshot://ocr"]), Route::Ocr);
        assert_eq!(
            route(&["open", "easyshot://settings"]),
            Route::Settings(None)
        );
        for denied in [
            "easyshot://quit",
            "easyshot://settings/general",
            "easyshot://capture/region?copy=1",
            "easyshot://capture/region#x",
            "easyshot://user@capture/region",
            "easyshot://capture/previous",
            "https://capture/region",
            "file:///etc/passwd",
        ] {
            assert!(request(&["open", denied]).is_err(), "{denied}");
        }
        assert!(request(&["open", "/definitely/not/here.png"]).is_err());
    }

    #[test]
    fn open_canonicalises_an_image_path() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        std::fs::write(&png, [0x89, b'P', b'N', b'G', 0]).unwrap();
        let sneaky = dir.path().join(".").join("a.png");
        assert_eq!(
            route(&["open", sneaky.to_str().unwrap()]),
            Route::Open(png.canonicalize().unwrap())
        );
        let text = dir.path().join("a.txt");
        std::fs::write(&text, b"hi").unwrap();
        assert!(request(&["open", text.to_str().unwrap()]).is_err());
    }

    #[test]
    fn doctor_and_daemon_are_not_routes() {
        assert_eq!(
            request(&["doctor", "--json", "--bundle"]).unwrap(),
            Request::Doctor {
                json: true,
                bundle: true
            }
        );
        assert_eq!(request(&["daemon"]).unwrap(), Request::Daemon);
    }

    #[test]
    fn completions_are_not_routes() {
        for shell in [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
        ] {
            assert_eq!(
                request(&["completions", &shell.to_string()]).unwrap(),
                Request::Completions(shell)
            );
        }
    }
}
