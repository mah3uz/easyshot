use super::{AgentTarget, VERSION_ARG};
use crate::{ArgValue, RouteArgs};
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_SECONDS: u32 = 120;
pub const MAX_SECONDS: u32 = 600;
pub const AUDIO: [&str; 4] = ["system", "mic", "both", "none"];
pub const FPS: [u32; 2] = [30, 60];
const MAX_ID: usize = 64;

const AUDIO_ARG: &str = "audio";
const FPS_ARG: &str = "fps";
const KEYSTROKES_ARG: &str = "keystrokes";
const MAX_SECONDS_ARG: &str = "max_seconds";
const ID_ARG: &str = "id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audio {
    System,
    Mic,
    Both,
    None,
}

impl Audio {
    pub fn name(self) -> &'static str {
        match self {
            Audio::System => AUDIO[0],
            Audio::Mic => AUDIO[1],
            Audio::Both => AUDIO[2],
            Audio::None => AUDIO[3],
        }
    }

    pub fn from_name(name: &str) -> Option<Audio> {
        [Audio::System, Audio::Mic, Audio::Both, Audio::None]
            .into_iter()
            .find(|a| a.name() == name)
    }
}

/// What one `agent record start` asks for; an omitted audio or fps takes the recording settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSpec {
    pub target: AgentTarget,
    pub audio: Option<Audio>,
    pub fps: Option<u32>,
    pub keystrokes: bool,
    pub max_seconds: u32,
}

impl RecordSpec {
    pub const fn new(target: AgentTarget) -> RecordSpec {
        RecordSpec {
            target,
            audio: None,
            fps: None,
            keystrokes: false,
            max_seconds: DEFAULT_MAX_SECONDS,
        }
    }

    fn write_args(&self, args: &mut RouteArgs) {
        self.target.write_args(args);
        let mut put = |k: &str, v: ArgValue| {
            args.0.insert(k.to_owned(), v);
        };
        if let Some(audio) = self.audio {
            put(AUDIO_ARG, ArgValue::Str(audio.name().to_owned()));
        }
        if let Some(fps) = self.fps {
            put(FPS_ARG, ArgValue::Int(i64::from(fps)));
        }
        put(KEYSTROKES_ARG, ArgValue::Bool(self.keystrokes));
        put(MAX_SECONDS_ARG, ArgValue::Int(i64::from(self.max_seconds)));
    }

    fn from_args(args: &RouteArgs) -> Result<RecordSpec, String> {
        let audio = match args.0.get(AUDIO_ARG) {
            None => None,
            Some(ArgValue::Str(s)) => Some(
                Audio::from_name(s)
                    .ok_or_else(|| format!("{AUDIO_ARG} is one of {}", AUDIO.join(", ")))?,
            ),
            Some(_) => return Err(format!("{AUDIO_ARG} must be a string")),
        };
        let fps = match args.0.get(FPS_ARG) {
            None => None,
            Some(ArgValue::Int(n)) => Some(
                u32::try_from(*n)
                    .ok()
                    .filter(|n| FPS.contains(n))
                    .ok_or_else(|| format!("{FPS_ARG} is 30 or 60"))?,
            ),
            Some(_) => return Err(format!("{FPS_ARG} must be a whole number")),
        };
        let keystrokes = match args.0.get(KEYSTROKES_ARG) {
            None => false,
            Some(ArgValue::Bool(b)) => *b,
            Some(_) => return Err(format!("{KEYSTROKES_ARG} must be true or false")),
        };
        let max_seconds = match args.0.get(MAX_SECONDS_ARG) {
            None => DEFAULT_MAX_SECONDS,
            Some(ArgValue::Int(n)) => u32::try_from(*n)
                .ok()
                .filter(|n| (1..=MAX_SECONDS).contains(n))
                .ok_or_else(|| format!("{MAX_SECONDS_ARG} is from 1 to {MAX_SECONDS}"))?,
            Some(_) => return Err(format!("{MAX_SECONDS_ARG} must be a whole number")),
        };
        Ok(RecordSpec {
            target: AgentTarget::from_args(args)?,
            audio,
            fps,
            keystrokes,
            max_seconds,
        })
    }
}

/// The recording routes. `Stop` and `Discard` name the recording by id, or mean the running one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordRoute {
    Start(RecordSpec),
    Stop(Option<String>),
    Status,
    Discard(Option<String>),
}

impl RecordRoute {
    pub(super) const NAMES: [&'static str; 4] = [
        "agent/record-start",
        "agent/record-stop",
        "agent/record-status",
        "agent/record-discard",
    ];

    fn index(&self) -> usize {
        match self {
            RecordRoute::Start(_) => 0,
            RecordRoute::Stop(_) => 1,
            RecordRoute::Status => 2,
            RecordRoute::Discard(_) => 3,
        }
    }

    pub fn route_name(&self) -> &'static str {
        Self::NAMES[self.index()]
    }

    pub fn command_args(&self) -> Vec<&'static str> {
        let verb = ["start", "stop", "status", "discard"][self.index()];
        vec!["agent", "record", verb]
    }

    pub fn target(&self) -> Option<&AgentTarget> {
        match self {
            RecordRoute::Start(spec) => Some(&spec.target),
            _ => None,
        }
    }

    pub(super) fn write_args(&self, args: &mut RouteArgs) {
        match self {
            RecordRoute::Start(spec) => spec.write_args(args),
            RecordRoute::Stop(Some(id)) | RecordRoute::Discard(Some(id)) => {
                args.0.insert(ID_ARG.to_owned(), ArgValue::Str(id.clone()));
            }
            _ => {}
        }
    }

    /// The version has already been checked; `name` is one of `NAMES`.
    pub(super) fn parse(name: &str, args: &RouteArgs) -> Result<RecordRoute, String> {
        let own: &[&str] = match name {
            "agent/record-start" => &[AUDIO_ARG, FPS_ARG, KEYSTROKES_ARG, MAX_SECONDS_ARG],
            "agent/record-stop" | "agent/record-discard" => &[ID_ARG],
            _ => &[],
        };
        let known = |k: &str| {
            k == VERSION_ARG
                || own.contains(&k)
                || (name == Self::NAMES[0] && AgentTarget::ARGS.contains(&k))
        };
        if let Some(extra) = args.0.keys().find(|k| !known(k)) {
            return Err(format!("unknown argument {extra}"));
        }
        let id = || match args.0.get(ID_ARG) {
            None => Ok(None),
            Some(ArgValue::Str(s)) => check_id(s).map(Some),
            Some(_) => Err(format!("{ID_ARG} must be a string")),
        };
        match name {
            "agent/record-start" => RecordSpec::from_args(args).map(RecordRoute::Start),
            "agent/record-stop" => id().map(RecordRoute::Stop),
            "agent/record-discard" => id().map(RecordRoute::Discard),
            _ => Ok(RecordRoute::Status),
        }
    }
}

pub fn check_id(text: &str) -> Result<String, String> {
    let fine = |c: char| c.is_ascii_alphanumeric() || c == '-';
    if text.is_empty() || text.len() > MAX_ID || !text.chars().all(fine) {
        return Err(format!(
            "a recording id has 1 to {MAX_ID} letters, digits or dashes"
        ));
    }
    Ok(text.to_owned())
}

/// What a recording route answers; each route fills the fields that apply to it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordingInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// `recording`, `stopping` or `idle`; `discarded` after a discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_seconds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
