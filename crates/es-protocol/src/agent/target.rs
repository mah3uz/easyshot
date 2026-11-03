use std::fmt;

use crate::{ArgValue, RouteArgs};

const MAX_OUTPUT_NAME: usize = 64;
const MAX_WINDOW_ID: usize = 128;
const MAX_APP_ID: usize = 255;
const MAX_TITLE: usize = 1024;
/// Far beyond any real layout, and small enough that `x + width` never overflows an `i32`.
const MAX_COORD: i32 = 1 << 20;
const MAX_EXTENT: u32 = 1 << 15;

const OUTPUT_ARG: &str = "output";
const X_ARG: &str = "x";
const Y_ARG: &str = "y";
const WIDTH_ARG: &str = "width";
const HEIGHT_ARG: &str = "height";
const WINDOW_ID_ARG: &str = "window_id";
const APP_ID_ARG: &str = "window_app_id";
const TITLE_ARG: &str = "window_title";
const FULLSCREEN_ARG: &str = "fullscreen";

/// A wl_output name such as `DP-1` or `HDMI-A-1`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutputName(String);

impl OutputName {
    pub fn new(name: &str) -> Result<OutputName, String> {
        let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':');
        if name.is_empty() || name.len() > MAX_OUTPUT_NAME {
            return Err(format!(
                "an output name has 1 to {MAX_OUTPUT_NAME} characters"
            ));
        }
        if name.starts_with('-') || !name.chars().all(allowed) {
            return Err(format!("{name:?} is not an output name"));
        }
        Ok(OutputName(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OutputName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A rectangle in global logical coordinates, written `x,y WxH` as grim and slurp do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Geometry {
    pub fn new(x: i64, y: i64, width: i64, height: i64) -> Result<Geometry, String> {
        let coord = |v: i64| {
            i32::try_from(v)
                .ok()
                .filter(|v| v.abs() <= MAX_COORD)
                .ok_or_else(|| format!("a position is within ±{MAX_COORD}"))
        };
        let extent = |v: i64| {
            u32::try_from(v)
                .ok()
                .filter(|v| (1..=MAX_EXTENT).contains(v))
                .ok_or_else(|| format!("a width or height is from 1 to {MAX_EXTENT}"))
        };
        Ok(Geometry {
            x: coord(x)?,
            y: coord(y)?,
            width: extent(width)?,
            height: extent(height)?,
        })
    }

    pub fn parse(text: &str) -> Result<Geometry, String> {
        let bad = || format!("{text:?} is not a geometry; write it as \"x,y WxH\"");
        let (pos, size) = text.split_once(' ').ok_or_else(bad)?;
        let (x, y) = pos.split_once(',').ok_or_else(bad)?;
        let (w, h) = size.split_once('x').ok_or_else(bad)?;
        let int = |s: &str| {
            let digits = s.strip_prefix('-').unwrap_or(s);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            s.parse::<i64>().map_err(|_| bad())
        };
        Geometry::new(int(x)?, int(y)?, int(w)?, int(h)?)
    }
}

impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{} {}x{}", self.x, self.y, self.width, self.height)
    }
}

/// Matched against the compositor's window list; an id is stable, an app id or title may match several.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowSelector {
    Id(String),
    AppId(String),
    Title(String),
}

impl WindowSelector {
    pub fn id(id: &str) -> Result<WindowSelector, String> {
        if id.is_empty() || id.len() > MAX_WINDOW_ID || !id.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(format!("{id:?} is not a window id"));
        }
        Ok(WindowSelector::Id(id.to_owned()))
    }

    pub fn app_id(app_id: &str) -> Result<WindowSelector, String> {
        text("an app id", app_id, MAX_APP_ID).map(WindowSelector::AppId)
    }

    pub fn title(title: &str) -> Result<WindowSelector, String> {
        text("a window title", title, MAX_TITLE).map(WindowSelector::Title)
    }

    fn arg(&self) -> (&'static str, &str) {
        match self {
            WindowSelector::Id(s) => (WINDOW_ID_ARG, s),
            WindowSelector::AppId(s) => (APP_ID_ARG, s),
            WindowSelector::Title(s) => (TITLE_ARG, s),
        }
    }
}

fn text(what: &str, value: &str, max: usize) -> Result<String, String> {
    if value.is_empty() || value.len() > max {
        return Err(format!("{what} has 1 to {max} bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{what} has no control characters"));
    }
    Ok(value.to_owned())
}

/// What an agent command acts on; exactly one per request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTarget {
    Fullscreen,
    Output(OutputName),
    Geometry(Geometry),
    Window(WindowSelector),
}

impl AgentTarget {
    pub const ARGS: [&'static str; 9] = [
        OUTPUT_ARG,
        X_ARG,
        Y_ARG,
        WIDTH_ARG,
        HEIGHT_ARG,
        WINDOW_ID_ARG,
        APP_ID_ARG,
        TITLE_ARG,
        FULLSCREEN_ARG,
    ];

    pub fn write_args(&self, args: &mut RouteArgs) {
        let mut put = |k: &str, v: ArgValue| {
            args.0.insert(k.to_owned(), v);
        };
        match self {
            AgentTarget::Fullscreen => put(FULLSCREEN_ARG, ArgValue::Bool(true)),
            AgentTarget::Output(name) => put(OUTPUT_ARG, ArgValue::Str(name.0.clone())),
            AgentTarget::Geometry(g) => {
                put(X_ARG, ArgValue::Int(i64::from(g.x)));
                put(Y_ARG, ArgValue::Int(i64::from(g.y)));
                put(WIDTH_ARG, ArgValue::Int(i64::from(g.width)));
                put(HEIGHT_ARG, ArgValue::Int(i64::from(g.height)));
            }
            AgentTarget::Window(w) => {
                let (key, value) = w.arg();
                put(key, ArgValue::Str(value.to_owned()));
            }
        }
    }

    /// Reads only the target's own keys; the route rejects any other key it does not know.
    pub fn from_args(args: &RouteArgs) -> Result<AgentTarget, String> {
        let present: Vec<&str> = Self::ARGS
            .into_iter()
            .filter(|k| args.0.contains_key(*k))
            .collect();
        let string = |k: &str| match args.0.get(k) {
            Some(ArgValue::Str(s)) => Ok(s.as_str()),
            _ => Err(format!("{k} must be a string")),
        };
        let int = |k: &str| match args.0.get(k) {
            Some(ArgValue::Int(n)) => Ok(*n),
            _ => Err(format!("{k} must be a whole number")),
        };
        match present.as_slice() {
            [FULLSCREEN_ARG] => match args.0.get(FULLSCREEN_ARG) {
                Some(ArgValue::Bool(true)) => Ok(AgentTarget::Fullscreen),
                _ => Err(format!("{FULLSCREEN_ARG} must be true")),
            },
            [OUTPUT_ARG] => OutputName::new(string(OUTPUT_ARG)?).map(AgentTarget::Output),
            [X_ARG, Y_ARG, WIDTH_ARG, HEIGHT_ARG] => {
                Geometry::new(int(X_ARG)?, int(Y_ARG)?, int(WIDTH_ARG)?, int(HEIGHT_ARG)?)
                    .map(AgentTarget::Geometry)
            }
            [WINDOW_ID_ARG] => WindowSelector::id(string(WINDOW_ID_ARG)?).map(AgentTarget::Window),
            [APP_ID_ARG] => WindowSelector::app_id(string(APP_ID_ARG)?).map(AgentTarget::Window),
            [TITLE_ARG] => WindowSelector::title(string(TITLE_ARG)?).map(AgentTarget::Window),
            [] => Err("no target: give an output, a geometry, a window or fullscreen".to_owned()),
            _ => Err(
                "give exactly one target: an output, a geometry, a window or fullscreen".to_owned(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_output_names_are_accepted() {
        for name in [
            "DP-1",
            "HDMI-A-1",
            "eDP-1",
            "HEADLESS-1",
            "WL-1",
            "Virtual-1",
            "DVI-I-2",
        ] {
            assert_eq!(OutputName::new(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn output_names_that_could_mean_something_else_are_refused() {
        let long = "A".repeat(MAX_OUTPUT_NAME + 1);
        for bad in [
            "", "DP 1", "a/b", "../x", "-DP-1", "DP-1\n", "DP-1;rm", "é", "$(x)", "*", &long,
        ] {
            assert!(OutputName::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn geometry_parses_the_grim_form_and_prints_it_back() {
        for text in ["10,20 300x400", "-1920,0 1920x1080", "0,-5 1x1"] {
            assert_eq!(Geometry::parse(text).unwrap().to_string(), text);
        }
        assert_eq!(
            Geometry::parse("10,20 300x400").unwrap(),
            Geometry {
                x: 10,
                y: 20,
                width: 300,
                height: 400
            }
        );
    }

    #[test]
    fn malformed_or_out_of_range_geometry_is_refused() {
        for bad in [
            "",
            "10,20",
            "10 20 300x400",
            "10,20 300 400",
            "10,20  300x400",
            " 10,20 300x400",
            "10,20 300x400 ",
            "10,20 0x400",
            "10,20 300x0",
            "10,20 -300x400",
            "10,20 +300x400",
            "+10,20 300x400",
            "1.5,20 300x400",
            "10,20 300X400",
            "10,20 300x400x5",
            "--1,0 1x1",
            "-,0 1x1",
            "0,0 32769x1",
            "1048577,0 1x1",
            "0,-1048577 1x1",
            "99999999999999999999,0 1x1",
        ] {
            assert!(Geometry::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn window_selectors_are_bounded_and_free_of_control_characters() {
        assert!(WindowSelector::id("0x55d0c2a1b2c0").is_ok());
        assert!(WindowSelector::id("12").is_ok());
        assert!(WindowSelector::app_id("org.mozilla.firefox").is_ok());
        assert!(WindowSelector::title("Inbox — Mail").is_ok());
        for bad in ["", "a b", "x\n", &"1".repeat(MAX_WINDOW_ID + 1)] {
            assert!(WindowSelector::id(bad).is_err(), "{bad:?}");
        }
        for bad in ["", "a\u{7}", "line\nbreak", &"t".repeat(MAX_TITLE + 1)] {
            assert!(WindowSelector::title(bad).is_err(), "{bad:?}");
        }
        assert!(WindowSelector::app_id(&"a".repeat(MAX_APP_ID + 1)).is_err());
    }

    #[test]
    fn every_target_round_trips_through_route_args() {
        let targets = [
            AgentTarget::Fullscreen,
            AgentTarget::Output(OutputName::new("DP-1").unwrap()),
            AgentTarget::Geometry(Geometry::parse("-10,20 300x400").unwrap()),
            AgentTarget::Window(WindowSelector::id("0xabc").unwrap()),
            AgentTarget::Window(WindowSelector::app_id("foot").unwrap()),
            AgentTarget::Window(WindowSelector::title("a title").unwrap()),
        ];
        for target in targets {
            let mut args = RouteArgs::default();
            target.write_args(&mut args);
            assert!(
                args.0
                    .keys()
                    .all(|k| AgentTarget::ARGS.contains(&k.as_str()))
            );
            assert_eq!(AgentTarget::from_args(&args).unwrap(), target);
        }
    }

    #[test]
    fn target_args_are_checked_again_on_the_daemon_side() {
        let with = |pairs: &[(&str, ArgValue)]| {
            let mut args = RouteArgs::default();
            for (k, v) in pairs {
                args.0.insert((*k).to_owned(), v.clone());
            }
            AgentTarget::from_args(&args)
        };
        let int = ArgValue::Int;
        let geometry = |w: i64| {
            vec![
                (X_ARG, int(0)),
                (Y_ARG, int(0)),
                (WIDTH_ARG, int(w)),
                (HEIGHT_ARG, int(10)),
            ]
        };
        assert!(with(&geometry(10)).is_ok());
        assert!(with(&geometry(0)).is_err());
        assert!(with(&geometry(i64::from(MAX_EXTENT) + 1)).is_err());
        assert!(with(&geometry(10)[..3]).is_err(), "a partial geometry");
        assert!(with(&[]).is_err());
        assert!(with(&[(OUTPUT_ARG, ArgValue::Str("DP 1".into()))]).is_err());
        assert!(with(&[(OUTPUT_ARG, int(1))]).is_err());
        assert!(with(&[(FULLSCREEN_ARG, ArgValue::Bool(false))]).is_err());
        assert!(with(&[(TITLE_ARG, ArgValue::Str("a\nb".into()))]).is_err());
        assert!(
            with(&[
                (OUTPUT_ARG, ArgValue::Str("DP-1".into())),
                (FULLSCREEN_ARG, ArgValue::Bool(true)),
            ])
            .is_err(),
            "two targets"
        );
    }
}
