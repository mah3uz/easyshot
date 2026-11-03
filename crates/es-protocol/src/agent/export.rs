use std::path::{Path, PathBuf};

use crate::{ArgValue, RouteArgs};

pub const FORMATS: [&str; 3] = ["mp4", "webm", "gif"];
pub const SIZES: [&str; 4] = ["original", "1080p", "720p", "480p"];
pub const QUALITIES: [&str; 3] = ["high", "balanced", "small"];
pub const LOOKS: [&str; 4] = ["natural", "vivid", "muted", "mono"];
pub const MAX_BRIGHTNESS_MILLI: i64 = 1000;
const MAX_PATH: usize = 4096;
const MAX_TRIM_MS: u64 = 100 * 3600 * 1000;

const INPUT_ARG: &str = "input";
const FORMAT_ARG: &str = "format";
const SIZE_ARG: &str = "size";
const QUALITY_ARG: &str = "quality";
const LOOK_ARG: &str = "look";
const BRIGHTNESS_ARG: &str = "brightness_milli";
const TRIM_START_ARG: &str = "trim_start_ms";
const TRIM_END_ARG: &str = "trim_end_ms";
const OUT_ARG: &str = "out";
const FORCE_ARG: &str = "force";

/// `a..b` in seconds; an omitted end is the start or end of the video.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrimRange {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
}

impl TrimRange {
    pub fn parse(text: &str) -> Result<TrimRange, String> {
        let bad = || format!("{text:?} is not a range of seconds such as 1.5..4 or ..10");
        let (a, b) = text.split_once("..").ok_or_else(bad)?;
        let range = TrimRange {
            start_ms: seconds(a).ok_or_else(bad)?,
            end_ms: seconds(b).ok_or_else(bad)?,
        };
        match (range.start_ms, range.end_ms) {
            (None, None) => Err(bad()),
            (Some(s), Some(e)) if s >= e => Err(format!("{text:?} ends before it starts")),
            _ => Ok(range),
        }
    }
}

/// `Some(None)` is an omitted end; `None` is text that is not a number of seconds.
fn seconds(text: &str) -> Option<Option<u64>> {
    if text.is_empty() {
        return Some(None);
    }
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !digits(whole) || !digits(frac) || frac.len() > 6 {
        return None;
    }
    let ms = (text.parse::<f64>().ok()? * 1000.).round() as u64;
    (ms <= MAX_TRIM_MS).then_some(Some(ms))
}

/// What one `agent export` asks for; an omitted option takes the trim viewer's default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSpec {
    pub input: PathBuf,
    pub format: Option<String>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub look: Option<String>,
    pub brightness_milli: i32,
    pub trim: TrimRange,
    /// An absolute path ending in the format's extension; beside the input when omitted.
    pub out: Option<PathBuf>,
    pub force: bool,
}

impl ExportSpec {
    pub const ARGS: [&'static str; 10] = [
        INPUT_ARG,
        FORMAT_ARG,
        SIZE_ARG,
        QUALITY_ARG,
        LOOK_ARG,
        BRIGHTNESS_ARG,
        TRIM_START_ARG,
        TRIM_END_ARG,
        OUT_ARG,
        FORCE_ARG,
    ];

    pub const fn new(input: PathBuf) -> ExportSpec {
        ExportSpec {
            input,
            format: None,
            size: None,
            quality: None,
            look: None,
            brightness_milli: 0,
            trim: TrimRange {
                start_ms: None,
                end_ms: None,
            },
            out: None,
            force: false,
        }
    }

    pub fn write_args(&self, args: &mut RouteArgs) {
        let mut put = |k: &str, v: ArgValue| {
            args.0.insert(k.to_owned(), v);
        };
        put(INPUT_ARG, path_value(&self.input));
        for (key, value) in [
            (FORMAT_ARG, &self.format),
            (SIZE_ARG, &self.size),
            (QUALITY_ARG, &self.quality),
            (LOOK_ARG, &self.look),
        ] {
            if let Some(v) = value {
                put(key, ArgValue::Str(v.clone()));
            }
        }
        put(
            BRIGHTNESS_ARG,
            ArgValue::Int(i64::from(self.brightness_milli)),
        );
        for (key, value) in [
            (TRIM_START_ARG, self.trim.start_ms),
            (TRIM_END_ARG, self.trim.end_ms),
        ] {
            if let Some(ms) = value {
                put(key, ArgValue::Int(ms as i64));
            }
        }
        if let Some(out) = &self.out {
            put(OUT_ARG, path_value(out));
        }
        put(FORCE_ARG, ArgValue::Bool(self.force));
    }

    pub fn from_args(args: &RouteArgs) -> Result<ExportSpec, String> {
        let input = match args.0.get(INPUT_ARG) {
            Some(ArgValue::Str(s)) => check_path(s)?,
            _ => return Err(format!("{INPUT_ARG} must be a file path")),
        };
        let choice = |key: &str, allowed: &[&str]| match args.0.get(key) {
            None => Ok(None),
            Some(ArgValue::Str(s)) if allowed.contains(&s.as_str()) => Ok(Some(s.clone())),
            Some(_) => Err(format!("{key} is one of {}", allowed.join(", "))),
        };
        let brightness_milli = match args.0.get(BRIGHTNESS_ARG) {
            None => 0,
            Some(ArgValue::Int(n)) if n.abs() <= MAX_BRIGHTNESS_MILLI => *n as i32,
            Some(_) => return Err(format!("{BRIGHTNESS_ARG} is from -1000 to 1000")),
        };
        let ms = |key: &str| match args.0.get(key) {
            None => Ok(None),
            Some(ArgValue::Int(n)) if (0..=MAX_TRIM_MS as i64).contains(n) => Ok(Some(*n as u64)),
            Some(_) => Err(format!("{key} must be a number of milliseconds")),
        };
        let trim = TrimRange {
            start_ms: ms(TRIM_START_ARG)?,
            end_ms: ms(TRIM_END_ARG)?,
        };
        if matches!((trim.start_ms, trim.end_ms), (Some(s), Some(e)) if s >= e) {
            return Err("the trim ends before it starts".to_owned());
        }
        let out = match args.0.get(OUT_ARG) {
            None => None,
            Some(ArgValue::Str(s)) => Some(check_path(s)?),
            Some(_) => return Err(format!("{OUT_ARG} must be a string")),
        };
        let force = match args.0.get(FORCE_ARG) {
            None => false,
            Some(ArgValue::Bool(b)) => *b,
            Some(_) => return Err(format!("{FORCE_ARG} must be true or false")),
        };
        let format = choice(FORMAT_ARG, &FORMATS)?;
        if let Some(out) = &out {
            let wanted = extension_format(out)?;
            if format.as_deref().is_some_and(|f| f != wanted) {
                return Err(format!(
                    "{} is not a .{} file",
                    out.display(),
                    format.unwrap_or_default()
                ));
            }
        }
        Ok(ExportSpec {
            input,
            format,
            size: choice(SIZE_ARG, &SIZES)?,
            quality: choice(QUALITY_ARG, &QUALITIES)?,
            look: choice(LOOK_ARG, &LOOKS)?,
            brightness_milli,
            trim,
            out,
            force,
        })
    }
}

fn path_value(path: &Path) -> ArgValue {
    ArgValue::Str(path.to_string_lossy().into_owned())
}

pub fn check_path(text: &str) -> Result<PathBuf, String> {
    if text.is_empty() || text.len() > MAX_PATH || text.contains('\0') {
        return Err(format!("a file path has 1 to {MAX_PATH} bytes and no NUL"));
    }
    let path = Path::new(text);
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(format!("{text:?} is not an absolute file path"));
    }
    Ok(path.to_path_buf())
}

/// The format a file name asks for.
pub fn extension_format(path: &Path) -> Result<&'static str, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase());
    FORMATS
        .into_iter()
        .find(|f| ext.as_deref() == Some(*f))
        .ok_or_else(|| format!("{} is not a .mp4, .webm or .gif file", path.display()))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
