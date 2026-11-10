use clap::{ArgGroup, Args};
use clap_complete::engine::ArgValueCompleter;
use es_protocol::agent::{
    AgentTarget, CaptureSpec, Geometry, MAX_DELAY_SECONDS, OutputName, WindowSelector, check_out,
};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["output", "geometry", "window", "fullscreen"])
))]
pub struct CaptureArgs {
    /// Capture one output, such as DP-1
    #[arg(long, value_name = "NAME", value_parser = parse_output)]
    output: Option<OutputName>,
    /// Capture a rectangle of the desktop, written "x,y WxH"
    #[arg(long, value_name = "AREA", value_parser = Geometry::parse, allow_hyphen_values = true)]
    geometry: Option<Geometry>,
    /// Capture a window, named by its id, its app id or its title
    #[arg(
        long,
        value_name = "WINDOW",
        value_parser = WindowSelector::any,
        allow_hyphen_values = true,
        add = ArgValueCompleter::new(crate::complete::window_ids)
    )]
    window: Option<WindowSelector>,
    /// Capture the focused output
    #[arg(long)]
    fullscreen: bool,
    /// Also write the PNG to this file; its folder must exist
    #[arg(long, value_name = "PATH", value_parser = parse_out)]
    out: Option<PathBuf>,
    /// Wait this many seconds before the capture
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 0,
        value_parser = clap::value_parser!(u8).range(0..=i64::from(MAX_DELAY_SECONDS))
    )]
    delay: u8,
    /// Show no deck card or notification; with --out the file is all that is written
    #[arg(long)]
    no_ui: bool,
    /// Wait for the capture to finish (the default)
    #[arg(long, conflicts_with = "no_wait")]
    wait: bool,
    /// Return once the daemon has accepted the capture
    #[arg(long)]
    no_wait: bool,
}

impl CaptureArgs {
    pub fn spec(&self) -> CaptureSpec {
        let target = if let Some(name) = &self.output {
            AgentTarget::Output(name.clone())
        } else if let Some(area) = self.geometry {
            AgentTarget::Geometry(area)
        } else if let Some(window) = &self.window {
            AgentTarget::Window(window.clone())
        } else {
            AgentTarget::Fullscreen
        };
        CaptureSpec {
            target,
            out: self.out.clone(),
            delay: self.delay,
            no_ui: self.no_ui,
        }
    }

    pub fn wait(&self) -> bool {
        !self.no_wait
    }
}

pub(super) fn parse_output(text: &str) -> Result<OutputName, String> {
    OutputName::new(text)
}

fn parse_out(text: &str) -> Result<PathBuf, String> {
    let path = std::path::absolute(text).map_err(|e| format!("{text:?}: {e}"))?;
    let path = check_out(&path.to_string_lossy())?;
    match path.parent().map(std::path::Path::is_dir) {
        Some(true) => Ok(path),
        _ => Err(format!("the folder of {text:?} does not exist")),
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
