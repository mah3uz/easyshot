use es_app::deck::{DeckItem, ItemStatus};
use es_model::CaptureKind;
use es_model::config::Config;
use es_model::recording::{RecordingCapture, RecordingInput, RecordingProject};
use es_record::project::{CommandRunner, POSTER, ProjectFolder};
use es_record::{Codec, Container, Quality, RecordingOutput, recovery};
use es_store::naming::{self, Kind};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn codec(name: &str) -> Codec {
    match name {
        "h265" | "hevc" => Codec::H265,
        "av1" => Codec::Av1,
        "vp9" => Codec::Vp9,
        _ => Codec::H264,
    }
}

pub fn container(name: &str) -> Container {
    match name {
        "mp4" => Container::Mp4,
        "webm" => Container::Webm,
        _ => Container::Mkv,
    }
}

pub fn quality(name: &str) -> Quality {
    match name {
        "medium" => Quality::Medium,
        "high" => Quality::High,
        "ultra" => Quality::Ultra,
        _ => Quality::VeryHigh,
    }
}

/// What the finished project records about how it was made.
pub struct Meta {
    pub elapsed: std::time::Duration,
    pub fps: u32,
    pub codec: String,
    pub container: String,
    pub recorder: String,
    pub output: String,
    pub system_audio: bool,
    pub mic: bool,
    pub created_at: OffsetDateTime,
}

pub struct Completed {
    pub item: DeckItem,
    pub capture: RecordingCapture,
}

fn stem(dir: &Path) -> String {
    dir.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn card(dir: &Path, video: PathBuf, kind: CaptureKind) -> DeckItem {
    DeckItem {
        id: Uuid::new_v4(),
        name: stem(dir),
        file: video,
        thumb: dir.join(POSTER),
        kind,
        video: true,
        staged: false,
        status: ItemStatus::Idle,
        auto_dismiss: true,
        saved: None,
    }
}

/// The clicks and keys read during the recording, on its timeline; empty when none were read.
pub fn input(
    read: Option<es_record::Result<es_record::InputBuilder>>,
    out: &RecordingOutput,
) -> RecordingInput {
    match read {
        Some(Ok(builder)) => builder.finish(out),
        Some(Err(e)) => {
            tracing::warn!(error = %e, "clicks and keys lost");
            RecordingInput::default()
        }
        None => RecordingInput::default(),
    }
}

/// Writes the project files next to the video and returns its deck card. Runs off the UI thread.
pub fn complete(
    folder: &ProjectFolder,
    out: &RecordingOutput,
    meta: &Meta,
    input: &RecordingInput,
    kind: CaptureKind,
    runner: &dyn CommandRunner,
) -> es_record::Result<Completed> {
    if out.split_audio
        && let Err(e) = es_record::mute::merge_tracks(runner, &out.file)
    {
        tracing::warn!(error = %e, "the two sound tracks stay separate");
    }
    let probe = recovery::probe_video(runner, &out.file);
    let capture = RecordingCapture {
        created_at: meta.created_at,
        duration_secs: probe
            .as_ref()
            .map_or(meta.elapsed.as_secs_f64(), |p| p.duration_secs),
        pixel_width: probe.as_ref().map_or(0, |p| p.width),
        pixel_height: probe.as_ref().map_or(0, |p| p.height),
        source_output: meta.output.clone(),
        video_file: out
            .file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        fps: meta.fps,
        codec: meta.codec.clone(),
        container: meta.container.clone(),
        recorder: meta.recorder.clone(),
        includes_system_audio: meta.system_audio,
        includes_microphone: meta.mic,
        ..RecordingCapture::default()
    };
    let project = RecordingProject {
        display_name: stem(folder.dir()),
        saved_at: meta.created_at,
        file_name: capture.video_file.clone(),
        ..RecordingProject::default()
    };
    folder.finish(&capture, &project, input, runner)?;
    Ok(Completed {
        item: card(folder.dir(), out.file.clone(), kind),
        capture,
    })
}

/// The card for a recording that recovery finished after a crash.
pub fn recovered_card(r: &recovery::Recovered) -> DeckItem {
    let video = r.dir.join(es_record::project::VIDEO);
    card(&r.dir, video, CaptureKind::Fullscreen)
}

/// Copies the streams into an mp4 that starts playing before it is fully downloaded.
pub fn remux_args(video: &Path, target: &Path) -> Vec<String> {
    [
        "-y",
        "-v",
        "error",
        "-i",
        &video.to_string_lossy(),
        "-c",
        "copy",
        "-movflags",
        "+faststart",
        &target.to_string_lossy(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// A dated name in the videos folder that no earlier file has.
pub fn video_target(
    paths: &es_store::Paths,
    template: &str,
    ext: &str,
    now: OffsetDateTime,
) -> Result<PathBuf, String> {
    let dir = paths.videos_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut scratch = es_model::State::default();
    let name = naming::next_file_name(template, ext, Kind::Recording, now, &mut scratch);
    Ok(naming::unique_path(&name, &dir, "-", naming::exists))
}

/// Remuxes into the videos folder; returns the new file.
pub fn save_mp4(
    video: &Path,
    paths: &es_store::Paths,
    config: &Config,
    now: OffsetDateTime,
    runner: &dyn CommandRunner,
) -> Result<PathBuf, String> {
    let target = video_target(paths, &config.general.file_name_template, "mp4", now)?;
    let out = runner
        .run("ffmpeg", &remux_args(video, &target))
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if out.status.success() && target.is_file() {
        Ok(target)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}
