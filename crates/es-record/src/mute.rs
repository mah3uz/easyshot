use crate::project::{CommandRunner, io_err};
use crate::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioSource {
    System,
    Mic,
}

/// The sources a recording takes sound from, in the order the recorder opens them.
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

/// How a recorder's capture streams in the sound server line up with the sources it records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// One stream per source, in source order: each can be muted alone.
    PerSource,
    /// Fewer streams than sources: the sources were mixed before the sound server saw them.
    Merged,
    Missing,
}

impl Shape {
    pub fn of(streams: usize, sources: usize) -> Self {
        match streams {
            0 => Self::Missing,
            n if n >= sources => Self::PerSource,
            _ => Self::Merged,
        }
    }
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

/// The streams that carry `source`.
pub fn targets(streams: &[u32], sources: &[AudioSource], source: AudioSource) -> Result<Vec<u32>> {
    match Shape::of(streams.len(), sources.len()) {
        Shape::Missing => Err(Error::NotFound("the recorder's sound streams".into())),
        Shape::Merged => Err(Error::Unsupported),
        Shape::PerSource => sources
            .iter()
            .position(|s| *s == source)
            .and_then(|i| streams.get(i))
            .map(|id| vec![*id])
            .ok_or_else(|| Error::NotFound("a stream for that source".into())),
    }
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

fn merged_path(file: &Path) -> PathBuf {
    let ext = file.extension().map(|e| e.to_string_lossy().into_owned());
    file.with_extension(format!("merged.{}", ext.unwrap_or_else(|| "mkv".into())))
}

/// Mixes the first two sound tracks into one and keeps the video as it is.
pub fn merge_args(file: &Path, out: &Path) -> Vec<String> {
    let codec = match file.extension().and_then(|e| e.to_str()) {
        Some("mp4") => "aac",
        _ => "libopus",
    };
    args(&[
        "-y",
        "-v",
        "error",
        "-i",
        &file.to_string_lossy(),
        "-filter_complex",
        "[0:a:0][0:a:1]amix=inputs=2:duration=longest:normalize=0[a]",
        "-map",
        "0:v",
        "-map",
        "[a]",
        "-c:v",
        "copy",
        "-c:a",
        codec,
        &out.to_string_lossy(),
    ])
}

/// Replaces a two-track recording with the same video and its two sound tracks mixed into one.
pub fn merge_tracks(runner: &dyn CommandRunner, file: &Path) -> Result<()> {
    let out = merged_path(file);
    let res = runner
        .run("ffmpeg", &merge_args(file, &out))
        .map_err(|e| io_err(file, e))?;
    if !res.status.success() || !out.is_file() {
        let _ = std::fs::remove_file(&out);
        return Err(Error::Protocol(format!(
            "ffmpeg track merge failed: {}",
            String::from_utf8_lossy(&res.stderr).trim()
        )));
    }
    std::fs::rename(&out, file).map_err(|e| io_err(file, e))
}
