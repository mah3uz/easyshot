use clap::Args;
use clap::builder::PossibleValuesParser;
use es_protocol::agent::{
    ExportSpec, FORMATS, LOOKS, MAX_BRIGHTNESS_MILLI, QUALITIES, SIZES, TrimRange, check_path,
    extension_format,
};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// The recording to export
    #[arg(value_name = "VIDEO", value_parser = parse_path)]
    input: PathBuf,
    /// The file type (default: that of --out, else mp4)
    #[arg(long, value_name = "FORMAT", value_parser = PossibleValuesParser::new(FORMATS))]
    format: Option<String>,
    /// The height to scale down to; a smaller video is never enlarged
    #[arg(long, value_name = "SIZE", value_parser = PossibleValuesParser::new(SIZES))]
    size: Option<String>,
    #[arg(long, value_name = "QUALITY", value_parser = PossibleValuesParser::new(QUALITIES))]
    quality: Option<String>,
    /// Keep only the seconds from A to B; either end may be left out, as in 2.5..8 or ..10
    #[arg(long, value_name = "A..B", value_parser = TrimRange::parse, allow_hyphen_values = true)]
    trim: Option<TrimRange>,
    /// A colour look
    #[arg(long, value_name = "LOOK", value_parser = PossibleValuesParser::new(LOOKS))]
    look: Option<String>,
    /// From -1 (darker) to 1 (brighter)
    #[arg(long, value_name = "AMOUNT", value_parser = parse_brightness, allow_hyphen_values = true)]
    brightness: Option<i32>,
    /// Write the file here; its folder must exist (default: beside the video)
    #[arg(long, value_name = "PATH", value_parser = parse_out)]
    out: Option<PathBuf>,
    /// Replace the file if it already exists
    #[arg(long)]
    force: bool,
    /// Wait for the export to finish (the default)
    #[arg(long, conflicts_with = "no_wait")]
    wait: bool,
    /// Return once the daemon has accepted the export
    #[arg(long)]
    no_wait: bool,
}

impl ExportArgs {
    pub fn spec(&self) -> ExportSpec {
        ExportSpec {
            format: self.format.clone(),
            size: self.size.clone(),
            quality: self.quality.clone(),
            look: self.look.clone(),
            brightness_milli: self.brightness.unwrap_or(0),
            trim: self.trim.unwrap_or_default(),
            out: self.out.clone(),
            force: self.force,
            ..ExportSpec::new(self.input.clone())
        }
    }

    pub fn wait(&self) -> bool {
        !self.no_wait
    }
}

fn parse_path(text: &str) -> Result<PathBuf, String> {
    let path = std::path::absolute(text).map_err(|e| format!("{text:?}: {e}"))?;
    check_path(&path.to_string_lossy())
}

fn parse_out(text: &str) -> Result<PathBuf, String> {
    let path = parse_path(text)?;
    extension_format(&path)?;
    match path.parent().map(std::path::Path::is_dir) {
        Some(true) => Ok(path),
        _ => Err(format!("the folder of {text:?} does not exist")),
    }
}

fn parse_brightness(text: &str) -> Result<i32, String> {
    let bad = || format!("{text:?} is not a number from -0.3 to 0.3");
    let value: f64 = text.parse().map_err(|_| bad())?;
    let milli = (value * 1000.).round();
    if !value.is_finite() || milli.abs() > MAX_BRIGHTNESS_MILLI as f64 {
        return Err(bad());
    }
    Ok(milli as i32)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
