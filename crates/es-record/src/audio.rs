use crate::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    SystemOutput,
    Microphone,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub kind: AudioKind,
}

#[derive(Deserialize)]
struct Source {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    properties: HashMap<String, serde_json::Value>,
}

pub fn parse_pactl_sources(json: &str) -> Result<Vec<AudioDevice>> {
    let sources: Vec<Source> = serde_json::from_str(json).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })?;
    let mut out = vec![
        AudioDevice {
            name: "default_output".into(),
            description: "Default output".into(),
            kind: AudioKind::SystemOutput,
        },
        AudioDevice {
            name: "default_input".into(),
            description: "Default input".into(),
            kind: AudioKind::Microphone,
        },
    ];
    out.extend(sources.into_iter().map(|s| {
        let class = s.properties.get("device.class").and_then(|v| v.as_str());
        let kind = match class {
            Some("monitor") => AudioKind::SystemOutput,
            Some("sound") => AudioKind::Microphone,
            _ => AudioKind::Virtual,
        };
        AudioDevice {
            name: s.name,
            description: s.description,
            kind,
        }
    }));
    Ok(out)
}

pub trait Runner {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<String>;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> std::io::Result<String> {
        let out = std::process::Command::new(program).args(args).output()?;
        if !out.status.success() {
            return Err(std::io::Error::other(format!(
                "{program} exited with {}",
                out.status
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

pub fn list_audio_devices(runner: &dyn Runner) -> Result<Vec<AudioDevice>> {
    let json = runner
        .run("pactl", &["-f", "json", "list", "sources"])
        .map_err(|_| Error::Unsupported)?;
    parse_pactl_sources(&json)
}
