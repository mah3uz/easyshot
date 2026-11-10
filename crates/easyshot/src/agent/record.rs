use super::capture::parse_output;
use clap::builder::PossibleValuesParser;
use clap::{ArgGroup, Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use es_protocol::agent::{
    AUDIO, AgentTarget, Audio, DEFAULT_MAX_SECONDS, FPS, Geometry, MAX_SECONDS, OutputName,
    RecordRoute, RecordSpec, WindowSelector, check_id,
};

#[derive(Subcommand, Debug)]
pub enum RecordCommand {
    /// Start recording one target and print the recording's id
    Start(StartArgs),
    /// Stop a recording and print where the video was saved
    Stop(IdArgs),
    /// Show whether a recording is running
    Status,
    /// Stop a recording and delete it
    Discard(IdArgs),
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .multiple(false)
        .args(["output", "geometry", "window", "fullscreen"])
))]
pub struct StartArgs {
    /// Record one output, such as DP-1
    #[arg(long, value_name = "NAME", value_parser = parse_output)]
    output: Option<OutputName>,
    /// Record a rectangle of the desktop, written "x,y WxH"
    #[arg(long, value_name = "AREA", value_parser = Geometry::parse, allow_hyphen_values = true)]
    geometry: Option<Geometry>,
    /// Record a window, named by its id, its app id or its title
    #[arg(
        long,
        value_name = "WINDOW",
        value_parser = WindowSelector::any,
        allow_hyphen_values = true,
        add = ArgValueCompleter::new(crate::complete::window_ids)
    )]
    window: Option<WindowSelector>,
    /// Record the focused output
    #[arg(long)]
    fullscreen: bool,
    /// Which sound to record (default: the recording settings)
    #[arg(long, value_name = "SOUND", value_parser = PossibleValuesParser::new(AUDIO))]
    audio: Option<String>,
    /// Frames per second (default: the recording setting)
    #[arg(long, value_name = "FPS", value_parser = parse_fps)]
    fps: Option<u32>,
    /// Record clicks and keys for the editor, whatever the setting says
    #[arg(long)]
    keystrokes: bool,
    /// Stop by itself after this many seconds
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = DEFAULT_MAX_SECONDS,
        value_parser = clap::value_parser!(u32).range(1..=i64::from(MAX_SECONDS))
    )]
    max_seconds: u32,
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// The recording's id (default: the one running)
    #[arg(long, value_name = "ID", value_parser = check_id)]
    id: Option<String>,
}

fn parse_fps(text: &str) -> Result<u32, String> {
    text.parse()
        .ok()
        .filter(|n| FPS.contains(n))
        .ok_or_else(|| format!("{text:?} is not 30 or 60"))
}

impl RecordCommand {
    pub fn route(&self) -> RecordRoute {
        match self {
            RecordCommand::Start(args) => RecordRoute::Start(args.spec()),
            RecordCommand::Stop(args) => RecordRoute::Stop(args.id.clone()),
            RecordCommand::Status => RecordRoute::Status,
            RecordCommand::Discard(args) => RecordRoute::Discard(args.id.clone()),
        }
    }
}

impl StartArgs {
    fn spec(&self) -> RecordSpec {
        let target = if let Some(name) = &self.output {
            AgentTarget::Output(name.clone())
        } else if let Some(area) = self.geometry {
            AgentTarget::Geometry(area)
        } else if let Some(window) = &self.window {
            AgentTarget::Window(window.clone())
        } else {
            AgentTarget::Fullscreen
        };
        RecordSpec {
            target,
            audio: self.audio.as_deref().and_then(Audio::from_name),
            fps: self.fps,
            keystrokes: self.keystrokes,
            max_seconds: self.max_seconds,
        }
    }
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
