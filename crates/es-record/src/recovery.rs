use crate::Result;
use crate::project::{
    CAPTURE, CommandRunner, FOLDER_SUFFIX, PROJECT, ProjectFolder, VIDEO, io_err, make_poster,
};
use es_model::recording::{RecordingCapture, RecordingProject};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

pub const MIN_DURATION_SECS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoProbe {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    pub dir: PathBuf,
    pub capture: RecordingCapture,
}

#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub recovered: Vec<Recovered>,
    pub unplayable: Vec<PathBuf>,
}

pub fn parse_probe(json: &str) -> Option<VideoProbe> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let stream = v.get("streams")?.as_array()?.first()?;
    let num = |x: &serde_json::Value| x.as_f64().or_else(|| x.as_str()?.parse().ok());
    Some(VideoProbe {
        duration_secs: num(v.get("format")?.get("duration")?)?,
        width: stream.get("width")?.as_u64()? as u32,
        height: stream.get("height")?.as_u64()? as u32,
        codec: stream.get("codec_name")?.as_str()?.to_owned(),
    })
}

pub fn probe_video(runner: &dyn CommandRunner, file: &Path) -> Option<VideoProbe> {
    let args: Vec<String> = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,codec_name:format=duration",
        "-of",
        "json",
        &file.to_string_lossy(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let out = runner.run("ffprobe", &args).ok()?;
    if !out.status.success() {
        return None;
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

fn is_session_dir(p: &Path) -> bool {
    p.is_dir() && p.extension().is_some_and(|e| e == FOLDER_SUFFIX)
}

/// Unopenable footage is reported and left in place, never deleted.
pub fn recover(recordings_dir: &Path, runner: &dyn CommandRunner) -> Result<RecoveryReport> {
    let mut report = RecoveryReport::default();
    let rd = match std::fs::read_dir(recordings_dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(report),
        Err(e) => return Err(io_err(recordings_dir, e)),
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| is_session_dir(p))
        .collect();
    dirs.sort();
    for dir in dirs {
        if dir.join(CAPTURE).exists() {
            continue;
        }
        let video = dir.join(VIDEO);
        let probe = video
            .is_file()
            .then(|| probe_video(runner, &video))
            .flatten()
            .filter(|p| p.duration_secs > MIN_DURATION_SECS);
        let Some(probe) = probe else {
            report.unplayable.push(dir);
            continue;
        };
        let created_at = std::fs::metadata(&video)
            .and_then(|m| m.modified())
            .map(OffsetDateTime::from)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        let capture = RecordingCapture {
            created_at,
            duration_secs: probe.duration_secs,
            pixel_width: probe.width,
            pixel_height: probe.height,
            codec: probe.codec,
            ..RecordingCapture::default()
        };
        let folder = ProjectFolder::open(&dir);
        if !dir.join(PROJECT).exists() {
            let name = dir
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            folder.write_project(&RecordingProject {
                display_name: name,
                saved_at: created_at,
                ..RecordingProject::default()
            })?;
        }
        if let Err(e) = make_poster(runner, &video, capture.duration_secs) {
            tracing::warn!(error = %e, "poster frame failed during recovery");
        }
        folder.write_capture(&capture)?;
        report.recovered.push(Recovered { dir, capture });
    }
    Ok(report)
}
