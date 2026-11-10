use crate::process::Clock;
use crate::project::CommandRunner;
use crate::{Error, RecorderEvent, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioSource {
    System,
    Mic,
}

/// The sources a recording takes sound from, one track each, in the order the recorder opens them:
/// system sound first, microphone second.
pub fn sources(system: bool, mic: bool) -> Vec<AudioSource> {
    let mut out = Vec::new();
    if system {
        out.push(AudioSource::System);
    }
    if mic {
        out.push(AudioSource::Mic);
    }
    out
}

#[derive(Deserialize)]
struct SourceOutput {
    index: u32,
    #[serde(default)]
    properties: HashMap<String, serde_json::Value>,
}

fn pid_of(props: &HashMap<String, serde_json::Value>) -> Option<u32> {
    match props.get("application.process.id")? {
        serde_json::Value::String(s) => s.parse().ok(),
        serde_json::Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

/// The source outputs of `pactl -f json list source-outputs` that process `pid` opened, oldest first.
pub fn parse_streams(json: &str, pid: u32) -> Result<Vec<u32>> {
    let all: Vec<SourceOutput> = serde_json::from_str(json).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })?;
    let mut ids: Vec<u32> = all
        .into_iter()
        .filter(|s| pid_of(&s.properties) == Some(pid))
        .map(|s| s.index)
        .collect();
    ids.sort_unstable();
    Ok(ids)
}

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn pactl(runner: &dyn CommandRunner, items: &[&str]) -> Result<String> {
    let out = runner
        .run("pactl", &args(items))
        .map_err(|_| Error::Unsupported)?;
    if !out.status.success() {
        return Err(Error::Protocol(format!(
            "pactl {}: {}",
            items.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn streams_of(runner: &dyn CommandRunner, pid: u32) -> Result<Vec<u32>> {
    parse_streams(
        &pactl(runner, &["-f", "json", "list", "source-outputs"])?,
        pid,
    )
}

/// The names of the sound server's sources in `pactl list short sources`.
pub fn parse_source_names(short: &str) -> Vec<String> {
    short
        .lines()
        .filter_map(|l| l.split('\t').nth(1))
        .map(str::to_owned)
        .collect()
}

/// The sound server's source names right now.
pub fn source_names(runner: &dyn CommandRunner) -> Result<Vec<String>> {
    Ok(parse_source_names(&pactl(
        runner,
        &["list", "short", "sources"],
    )?))
}

/// The sound server's name for the microphone a recording asked for, when it is there now.
pub fn mic_source(runner: &dyn CommandRunner, mic: &str) -> Option<String> {
    let wanted = match mic {
        "" | "default" | "default_input" => pactl(runner, &["get-default-source"])
            .ok()?
            .trim()
            .to_owned(),
        "default_output" => return None,
        m => m.strip_prefix("device:").unwrap_or(m).to_owned(),
    };
    source_names(runner)
        .ok()?
        .contains(&wanted)
        .then_some(wanted)
}

/// The stream that carries `source`: the recorder opens one per source, in the order of `sources`.
pub fn targets(streams: &[u32], sources: &[AudioSource], source: AudioSource) -> Result<Vec<u32>> {
    sources
        .iter()
        .position(|s| *s == source)
        .and_then(|i| streams.get(i))
        .map(|id| vec![*id])
        .ok_or_else(|| Error::NotFound("a stream for that source".into()))
}

/// Mutes a recorder's own capture streams; the user's devices and other apps are never touched.
#[derive(Clone)]
pub struct Mutes {
    runner: Arc<dyn CommandRunner>,
    sources: Vec<AudioSource>,
}

impl Mutes {
    pub fn new(runner: Arc<dyn CommandRunner>, sources: Vec<AudioSource>) -> Self {
        Self { runner, sources }
    }

    pub fn streams(&self, pid: u32) -> Result<Vec<u32>> {
        streams_of(&*self.runner, pid)
    }

    pub fn set(&self, pid: u32, source: AudioSource, muted: bool) -> Result<()> {
        let streams = self.streams(pid)?;
        let flag = if muted { "1" } else { "0" };
        for id in targets(&streams, &self.sources, source)? {
            pactl(
                &*self.runner,
                &["set-source-output-mute", &id.to_string(), flag],
            )?;
        }
        Ok(())
    }
}

pub(crate) const MIC_POLLS: u32 = 20;

pub(crate) fn mic_arg(mic: &str) -> String {
    match mic {
        "" | "default" | "default_input" => "default_input".into(),
        m if m.starts_with("device:") || m == "default_output" => m.into(),
        m => format!("device:{m}"),
    }
}

pub(crate) fn watch_mic(
    runner: Arc<dyn CommandRunner>,
    name: String,
    clock: Arc<dyn Clock>,
    every: Duration,
    stopping: Arc<AtomicBool>,
    tx: async_channel::Sender<RecorderEvent>,
) {
    std::thread::spawn(move || {
        loop {
            clock.sleep(every);
            if stopping.load(Ordering::SeqCst) || tx.is_closed() {
                return;
            }
            if source_names(&*runner).is_ok_and(|names| !names.contains(&name)) {
                let _ = tx.try_send(RecorderEvent::MicLost);
                return;
            }
        }
    });
}
