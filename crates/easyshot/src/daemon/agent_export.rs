use super::hints;
use es_protocol::agent::{
    AgentReply, ErrorCode, ExportInfo, ExportSpec, ReplyKind, extension_format,
};
use es_record::clip::export::{ExportError, ExportJob, ExportMode, FfmpegRunner, SystemFfmpeg};
use es_record::clip::options::{ExportOptions, Format, Look, Quality, Size};
use es_record::clip::trim::{MIN_TRIM_SECS, Trim};
use es_record::project::{CommandRunner, SystemRunner};
use es_record::recovery::{audio_track_count, probe_video};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Fail(ErrorCode, String, Option<String>);

fn fail(code: ErrorCode, message: impl Into<String>) -> Fail {
    Fail(code, message.into(), None)
}

pub(super) fn export(spec: &ExportSpec) -> AgentReply {
    match run(spec, &SystemRunner, &SystemFfmpeg) {
        Ok(info) => AgentReply {
            export: Some(Box::new(info)),
            ..AgentReply::ok(ReplyKind::Export)
        },
        Err(Fail(code, message, hint)) => {
            AgentReply::failed(ReplyKind::Export, code, message).hinted(hint)
        }
    }
}

fn run(
    spec: &ExportSpec,
    probe: &dyn CommandRunner,
    ffmpeg: &dyn FfmpegRunner,
) -> Result<ExportInfo, Fail> {
    check_input(&spec.input)?;
    if probe.run("ffprobe", &["-version".to_owned()]).is_err() {
        return Err(Fail(
            ErrorCode::Unsupported,
            "ffmpeg is not installed".to_owned(),
            Some(hints::ffmpeg_missing()),
        ));
    }
    let source = probe_video(probe, &spec.input).ok_or_else(|| {
        fail(
            ErrorCode::InvalidArguments,
            format!("{} is not a video easyshot can read", spec.input.display()),
        )
    })?;
    let mut job = job(spec, source.duration_secs)?;
    job.audio_tracks = audio_track_count(probe, &spec.input);
    es_record::clip::export::export(ffmpeg, &job, &mut |_| {}).map_err(|e| match e {
        ExportError::Io(e) => fail(ErrorCode::Failed, e.to_string()),
        ExportError::Ffmpeg(m) => fail(ErrorCode::Failed, format!("ffmpeg failed: {m}")),
    })?;
    let done = probe_video(probe, &job.output).ok_or_else(|| {
        fail(
            ErrorCode::Failed,
            "the exported file could not be read back",
        )
    })?;
    let bytes = std::fs::metadata(&job.output)
        .map_err(|e| fail(ErrorCode::Failed, e.to_string()))?
        .len();
    Ok(ExportInfo {
        path: job.output.display().to_string(),
        format: job.options.format.id().to_owned(),
        width: done.width,
        height: done.height,
        duration_ms: (done.duration_secs * 1000.).round() as u64,
        bytes,
    })
}

fn check_input(input: &Path) -> Result<(), Fail> {
    match std::fs::metadata(input) {
        Ok(m) if m.is_file() => std::fs::File::open(input)
            .map(drop)
            .map_err(|e| fail(ErrorCode::Failed, format!("{}: {e}", input.display()))),
        Ok(_) => Err(fail(
            ErrorCode::InvalidArguments,
            format!("{} is not a file", input.display()),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Fail(
            ErrorCode::NotFound,
            format!("no video at {}", input.display()),
            Some(crate::hints::file_missing(input)),
        )),
        Err(e) => Err(fail(ErrorCode::Failed, format!("{}: {e}", input.display()))),
    }
}

fn job(spec: &ExportSpec, duration: f64) -> Result<ExportJob, Fail> {
    let bad = |why: String| fail(ErrorCode::InvalidArguments, why);
    let format = spec
        .format
        .as_deref()
        .map(Ok)
        .or_else(|| spec.out.as_deref().map(extension_format))
        .transpose()
        .map_err(bad)?
        .map_or(Format::default(), Format::from_id);
    let options = ExportOptions {
        format,
        size: spec.size.as_deref().map_or(Size::default(), Size::from_id),
        quality: spec
            .quality
            .as_deref()
            .map_or(Quality::default(), Quality::from_id),
        look: spec.look.as_deref().map_or(Look::default(), Look::from_id),
        brightness: f64::from(spec.brightness_milli) / 1000.,
        ..ExportOptions::default()
    };
    let output = spec
        .out
        .clone()
        .unwrap_or_else(|| default_out(&spec.input, format));
    check_output(&output, &spec.input, spec.force)?;
    let mut trim = Trim::new(duration);
    let start = spec.trim.start_ms.map_or(0., |ms| ms as f64 / 1000.);
    let end = spec
        .trim
        .end_ms
        .map_or(duration, |ms| (ms as f64 / 1000.).min(duration));
    if end - start < MIN_TRIM_SECS {
        return Err(bad(format!(
            "the trim keeps less than {MIN_TRIM_SECS} seconds of a video that lasts {duration:.3}"
        )));
    }
    trim.set_end(end);
    trim.set_start(start);
    Ok(ExportJob {
        input: spec.input.clone(),
        output,
        trim,
        mode: ExportMode::Precise,
        remove_audio: false,
        audio_tracks: 0,
        options,
    })
}

fn default_out(input: &Path, format: Format) -> PathBuf {
    let stem = input
        .file_stem()
        .map_or_else(|| "video".into(), |s| s.to_string_lossy().into_owned());
    input.with_file_name(format!("{stem}-export.{}", format.id()))
}

fn check_output(out: &Path, input: &Path, force: bool) -> Result<(), Fail> {
    let bad = |why: String| fail(ErrorCode::InvalidArguments, why);
    if !out.parent().is_some_and(Path::is_dir) {
        return Err(bad(format!(
            "the folder of {} does not exist",
            out.display()
        )));
    }
    if out == input {
        return Err(bad("the output would replace the input".to_owned()));
    }
    if out.is_dir() {
        return Err(bad(format!("{} is a folder", out.display())));
    }
    if out.exists() && !force {
        return Err(bad(format!(
            "{} exists; give --force to replace it",
            out.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "agent_export_tests.rs"]
mod tests;
