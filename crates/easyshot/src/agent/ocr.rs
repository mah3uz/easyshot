use super::capture::parse_output;
use clap::{ArgGroup, Args};
use clap_complete::engine::ArgValueCompleter;
use es_protocol::agent::{
    AgentTarget, Geometry, OcrInput, OcrSpec, OutputName, WindowSelector, check_file,
    parse_languages,
};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("source")
        .required(true)
        .multiple(false)
        .args(["output", "geometry", "window", "fullscreen", "file"])
))]
pub struct OcrArgs {
    /// Read the text on one output, such as DP-1
    #[arg(long, value_name = "NAME", value_parser = parse_output)]
    output: Option<OutputName>,
    /// Read the text in a rectangle of the desktop, written "x,y WxH"
    #[arg(long, value_name = "AREA", value_parser = Geometry::parse, allow_hyphen_values = true)]
    geometry: Option<Geometry>,
    /// Read the text in a window, named by its id, its app id or its title
    #[arg(
        long,
        value_name = "WINDOW",
        value_parser = WindowSelector::any,
        allow_hyphen_values = true,
        add = ArgValueCompleter::new(crate::complete::window_ids)
    )]
    window: Option<WindowSelector>,
    /// Read the text on the focused output
    #[arg(long)]
    fullscreen: bool,
    /// Read the text in a PNG or JPEG file instead of the screen
    #[arg(long, value_name = "PATH", value_parser = parse_file)]
    file: Option<PathBuf>,
    /// Tesseract languages, such as eng+deu; the default is the setting [ocr] languages
    #[arg(long, value_name = "CODES", value_parser = checked_languages)]
    lang: Option<String>,
}

impl OcrArgs {
    pub fn spec(&self) -> OcrSpec {
        let input = if let Some(path) = &self.file {
            OcrInput::File(path.clone())
        } else if let Some(name) = &self.output {
            OcrInput::Target(AgentTarget::Output(name.clone()))
        } else if let Some(area) = self.geometry {
            OcrInput::Target(AgentTarget::Geometry(area))
        } else if let Some(window) = &self.window {
            OcrInput::Target(AgentTarget::Window(window.clone()))
        } else {
            OcrInput::Target(AgentTarget::Fullscreen)
        };
        OcrSpec {
            input,
            languages: self
                .lang
                .as_deref()
                .and_then(|text| parse_languages(text).ok())
                .unwrap_or_default(),
        }
    }
}

fn checked_languages(text: &str) -> Result<String, String> {
    parse_languages(text).map(|_| text.to_owned())
}

fn parse_file(text: &str) -> Result<PathBuf, String> {
    let path = std::path::absolute(text).map_err(|e| format!("{text:?}: {e}"))?;
    check_file(&path.to_string_lossy())
}

#[cfg(test)]
#[path = "ocr_tests.rs"]
mod tests;
