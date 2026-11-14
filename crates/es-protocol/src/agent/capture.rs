use std::path::{Path, PathBuf};

use super::AgentTarget;
use crate::{ArgValue, RouteArgs};

pub const MAX_DELAY_SECONDS: u8 = 60;
const MAX_OUT: usize = 4096;
const OUT_ARG: &str = "out";
const DELAY_ARG: &str = "delay";
const UI_ARG: &str = "ui";

/// What one `agent capture` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSpec {
    pub target: AgentTarget,
    /// An absolute path to a `.png` file whose folder exists.
    pub out: Option<PathBuf>,
    pub delay: u8,
    pub ui: bool,
}

impl CaptureSpec {
    pub const ARGS: [&'static str; 3] = [OUT_ARG, DELAY_ARG, UI_ARG];

    pub const fn new(target: AgentTarget) -> CaptureSpec {
        CaptureSpec {
            target,
            out: None,
            delay: 0,
            ui: false,
        }
    }

    pub fn write_args(&self, args: &mut RouteArgs) {
        self.target.write_args(args);
        let mut put = |k: &str, v: ArgValue| {
            args.0.insert(k.to_owned(), v);
        };
        if let Some(out) = &self.out {
            put(OUT_ARG, ArgValue::Str(out.to_string_lossy().into_owned()));
        }
        put(DELAY_ARG, ArgValue::Int(i64::from(self.delay)));
        put(UI_ARG, ArgValue::Bool(self.ui));
    }

    pub fn from_args(args: &RouteArgs) -> Result<CaptureSpec, String> {
        let out = match args.0.get(OUT_ARG) {
            None => None,
            Some(ArgValue::Str(s)) => Some(check_out(s)?),
            Some(_) => return Err(format!("{OUT_ARG} must be a string")),
        };
        let delay = match args.0.get(DELAY_ARG) {
            None => 0,
            Some(ArgValue::Int(n)) => u8::try_from(*n)
                .ok()
                .filter(|n| *n <= MAX_DELAY_SECONDS)
                .ok_or_else(|| format!("{DELAY_ARG} is from 0 to {MAX_DELAY_SECONDS} seconds"))?,
            Some(_) => return Err(format!("{DELAY_ARG} must be a whole number")),
        };
        let ui = match args.0.get(UI_ARG) {
            None => false,
            Some(ArgValue::Bool(b)) => *b,
            Some(_) => return Err(format!("{UI_ARG} must be true or false")),
        };
        Ok(CaptureSpec {
            target: AgentTarget::from_args(args)?,
            out,
            delay,
            ui,
        })
    }
}

pub fn check_out(text: &str) -> Result<PathBuf, String> {
    if text.is_empty() || text.len() > MAX_OUT || text.contains('\0') {
        return Err(format!("a file path has 1 to {MAX_OUT} bytes and no NUL"));
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(format!("{text:?} is not an absolute path"));
    }
    let png = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !png || path.file_name().is_none() {
        return Err(format!("{text:?} is not a .png file"));
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Geometry, OutputName};

    fn round_trip(spec: &CaptureSpec) -> Result<CaptureSpec, String> {
        let mut args = RouteArgs::default();
        spec.write_args(&mut args);
        CaptureSpec::from_args(&args)
    }

    #[test]
    fn every_option_survives_the_route_arguments() {
        let spec = CaptureSpec {
            target: AgentTarget::Geometry(Geometry::parse("0,0 64x48").unwrap()),
            out: Some(PathBuf::from("/tmp/shot.PNG")),
            delay: 60,
            ui: true,
        };
        assert_eq!(round_trip(&spec).unwrap(), spec);
        let plain = CaptureSpec::new(AgentTarget::Output(OutputName::new("DP-1").unwrap()));
        assert_eq!(round_trip(&plain).unwrap(), plain);
    }

    #[test]
    fn a_missing_option_takes_its_default() {
        let mut args = RouteArgs::default();
        AgentTarget::Fullscreen.write_args(&mut args);
        assert_eq!(
            CaptureSpec::from_args(&args).unwrap(),
            CaptureSpec::new(AgentTarget::Fullscreen)
        );
    }

    #[test]
    fn the_daemon_refuses_options_a_command_would_not_send() {
        let with = |key: &str, value: ArgValue| {
            let mut args = RouteArgs::default();
            AgentTarget::Fullscreen.write_args(&mut args);
            args.0.insert(key.to_owned(), value);
            CaptureSpec::from_args(&args)
        };
        let text = |s: &str| ArgValue::Str(s.to_owned());
        for bad in [
            "",
            "shot.png",
            "/tmp/shot.jpg",
            "/tmp/shot",
            "/tmp/",
            "/tmp/a\0.png",
            &format!("/{}.png", "a".repeat(MAX_OUT)),
        ] {
            assert!(with(OUT_ARG, text(bad)).is_err(), "{bad:?}");
        }
        assert!(with(OUT_ARG, ArgValue::Int(1)).is_err());
        assert!(with(DELAY_ARG, ArgValue::Int(i64::from(MAX_DELAY_SECONDS) + 1)).is_err());
        assert!(with(DELAY_ARG, ArgValue::Int(-1)).is_err());
        assert!(with(DELAY_ARG, text("3")).is_err());
        assert!(with(UI_ARG, text("true")).is_err());
        assert!(with(DELAY_ARG, ArgValue::Int(i64::from(MAX_DELAY_SECONDS))).is_ok());
    }

    #[test]
    fn exactly_one_target_is_still_required() {
        let mut args = RouteArgs::default();
        args.0.insert(UI_ARG.to_owned(), ArgValue::Bool(true));
        assert!(CaptureSpec::from_args(&args).is_err());
        AgentTarget::Fullscreen.write_args(&mut args);
        AgentTarget::Output(OutputName::new("DP-1").unwrap()).write_args(&mut args);
        assert!(CaptureSpec::from_args(&args).is_err());
    }
}
