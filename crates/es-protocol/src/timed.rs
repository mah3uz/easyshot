use crate::{ArgValue, Error, Result, Route, RouteArgs};

pub const SECONDS_ARG: &str = "seconds";
pub const MAX_TIMED_SECONDS: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedMode {
    Region,
    Window,
    Fullscreen,
}

impl TimedMode {
    pub const ALL: [TimedMode; 3] = [TimedMode::Region, TimedMode::Window, TimedMode::Fullscreen];

    pub fn route_name(self) -> &'static str {
        match self {
            TimedMode::Region => "capture/region-timed",
            TimedMode::Window => "capture/window-timed",
            TimedMode::Fullscreen => "capture/fullscreen-timed",
        }
    }

    /// The `easyshot capture` mode word.
    pub fn word(self) -> &'static str {
        match self {
            TimedMode::Region => "region",
            TimedMode::Window => "window",
            TimedMode::Fullscreen => "fullscreen",
        }
    }

    pub(crate) fn from_route_name(name: &str) -> Option<TimedMode> {
        Self::ALL.into_iter().find(|m| m.route_name() == name)
    }
}

impl Route {
    /// `capture timed` is the same request as a timed region with the self-timer setting.
    pub fn timed(mode: TimedMode, seconds: Option<u32>) -> Route {
        match (mode, seconds) {
            (TimedMode::Region, None) => Route::CaptureTimed,
            _ => Route::Timed { mode, seconds },
        }
    }

    pub(crate) fn timed_from_args(name: &str, mode: TimedMode, args: &RouteArgs) -> Result<Route> {
        let bad = |reason: String| Error::InvalidArgs {
            route: name.to_owned(),
            reason,
        };
        if args.0.keys().any(|k| k != SECONDS_ARG) {
            return Err(bad(format!("the only argument is {SECONDS_ARG}")));
        }
        let seconds = match args.0.get(SECONDS_ARG) {
            None => None,
            Some(ArgValue::Int(n)) => Some(
                u32::try_from(*n)
                    .ok()
                    .filter(|n| (1..=MAX_TIMED_SECONDS).contains(n))
                    .ok_or_else(|| {
                        bad(format!(
                            "{SECONDS_ARG} must be from 1 to {MAX_TIMED_SECONDS}"
                        ))
                    })?,
            ),
            Some(_) => return Err(bad(format!("{SECONDS_ARG} must be a whole number"))),
        };
        Ok(Route::timed(mode, seconds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(n: i64) -> RouteArgs {
        let mut a = RouteArgs::default();
        a.0.insert(SECONDS_ARG.into(), ArgValue::Int(n));
        a
    }

    #[test]
    fn timed_routes_round_trip_through_name_and_args() {
        for mode in TimedMode::ALL {
            for secs in [None, Some(1), Some(10), Some(MAX_TIMED_SECONDS)] {
                let r = Route::timed(mode, secs);
                assert_eq!(Route::parse(&r.as_str(), &r.args()).unwrap(), r, "{r:?}");
            }
        }
    }

    #[test]
    fn a_timed_region_with_no_seconds_is_capture_timed() {
        let r = Route::parse("capture/region-timed", &RouteArgs::default()).unwrap();
        assert_eq!(r, Route::CaptureTimed);
        assert_eq!(
            Route::parse("capture/window-timed", &RouteArgs::default()).unwrap(),
            Route::Timed {
                mode: TimedMode::Window,
                seconds: None
            }
        );
    }

    #[test]
    fn seconds_outside_one_to_sixty_or_other_arguments_are_refused() {
        for n in [0, -3, 61, i64::from(u32::MAX) + 1] {
            assert!(
                Route::parse("capture/window-timed", &seconds(n)).is_err(),
                "{n}"
            );
        }
        let mut extra = seconds(5);
        extra.0.insert("path".into(), ArgValue::Str("/x".into()));
        assert!(Route::parse("capture/fullscreen-timed", &extra).is_err());
        let mut text = RouteArgs::default();
        text.0.insert(SECONDS_ARG.into(), ArgValue::Str("5".into()));
        assert!(Route::parse("capture/region-timed", &text).is_err());
        assert!(Route::parse("capture/timed", &seconds(5)).is_err());
    }

    #[test]
    fn a_timed_command_line_names_the_mode_and_the_flag() {
        let r = Route::timed(TimedMode::Fullscreen, Some(5));
        assert_eq!(r.command_args(), ["capture", "fullscreen", "--timed"]);
        assert_eq!(r.args(), seconds(5));
    }

    #[test]
    fn timed_routes_are_not_links() {
        for mode in TimedMode::ALL {
            let url = format!("easyshot://{}", mode.route_name());
            assert!(Route::from_url(&url).is_err(), "{url}");
        }
    }
}
