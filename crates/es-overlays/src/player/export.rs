use super::trim::Trim;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExportMode {
    /// Re-encodes, so the cut lands on the exact frame.
    #[default]
    Precise,
    /// Copies the streams, so the start snaps to the keyframe at or before it.
    Fast,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportJob {
    pub input: PathBuf,
    pub output: PathBuf,
    pub trim: Trim,
    pub mode: ExportMode,
    pub remove_audio: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Ffmpeg(String),
}

pub struct RunOutcome {
    pub success: bool,
    pub stderr: String,
}

/// `on_line` sees each line ffmpeg writes to stdout as it is written.
pub trait FfmpegRunner: Send + Sync {
    fn run(&self, args: &[String], on_line: &mut dyn FnMut(&str)) -> std::io::Result<RunOutcome>;
}

pub struct SystemFfmpeg;

/// The child must never reach the user's display or session bus.
pub fn command(program: &str, args: &[String]) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

impl FfmpegRunner for SystemFfmpeg {
    fn run(&self, args: &[String], on_line: &mut dyn FnMut(&str)) -> std::io::Result<RunOutcome> {
        let mut child = command("ffmpeg", args).spawn()?;
        let stderr = child.stderr.take().map(|mut pipe| {
            std::thread::spawn(move || {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text);
                text
            })
        });
        if let Some(out) = child.stdout.take() {
            for line in BufReader::new(out).lines() {
                on_line(&line?);
            }
        }
        let status = child.wait()?;
        let stderr = stderr.and_then(|t| t.join().ok()).unwrap_or_default();
        Ok(RunOutcome {
            success: status.success(),
            stderr,
        })
    }
}

fn secs(t: f64) -> String {
    format!("{t:.3}")
}

pub fn export_args(job: &ExportJob, target: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["-y", "-v", "error", "-nostats", "-progress", "pipe:1"]
        .map(String::from)
        .to_vec();
    args.extend([
        "-ss".into(),
        secs(job.trim.start()),
        "-to".into(),
        secs(job.trim.end()),
        "-i".into(),
        job.input.to_string_lossy().into_owned(),
        "-map".into(),
        "0:v:0".into(),
    ]);
    if job.remove_audio {
        args.push("-an".into());
    } else {
        args.extend(["-map", "0:a?"].map(String::from));
    }
    let codecs: &[&str] = match job.mode {
        ExportMode::Precise => &[
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p", "-c:a",
            "aac", "-b:a", "192k",
        ],
        ExportMode::Fast => &["-c", "copy"],
    };
    args.extend(codecs.iter().map(|s| (*s).to_owned()));
    args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
    args.push(target.to_string_lossy().into_owned());
    args
}

/// Seconds of output written so far, from a `-progress` line.
pub fn progress_secs(line: &str) -> Option<f64> {
    let us: f64 = line.strip_prefix("out_time_us=")?.trim().parse().ok()?;
    Some((us / 1e6).max(0.))
}

fn partial(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .map_or_else(|| "export".into(), |n| n.to_string_lossy().into_owned());
    output.with_file_name(format!(".{name}.part"))
}

/// The file appears at `output` only once ffmpeg has finished it; `progress` gets 0..=1.
pub fn export(
    runner: &dyn FfmpegRunner,
    job: &ExportJob,
    progress: &mut dyn FnMut(f64),
) -> Result<PathBuf, ExportError> {
    let target = partial(&job.output);
    let kept = job.trim.kept().max(f64::EPSILON);
    let outcome = runner.run(&export_args(job, &target), &mut |line| {
        if let Some(t) = progress_secs(line) {
            progress((t / kept).min(1.));
        }
    });
    match outcome {
        Ok(o) if o.success && target.is_file() => {}
        Ok(o) => {
            let _ = std::fs::remove_file(&target);
            let reason = o
                .stderr
                .trim()
                .lines()
                .last()
                .unwrap_or_default()
                .to_owned();
            return Err(ExportError::Ffmpeg(reason));
        }
        Err(e) => {
            let _ = std::fs::remove_file(&target);
            return Err(e.into());
        }
    }
    std::fs::rename(&target, &job.output)?;
    progress(1.);
    Ok(job.output.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(mode: ExportMode, remove_audio: bool) -> ExportJob {
        let mut trim = Trim::new(5.);
        trim.set_start(1.);
        trim.set_end(3.5);
        ExportJob {
            input: "/v/screen.mkv".into(),
            output: "/out/clip.mp4".into(),
            trim,
            mode,
            remove_audio,
        }
    }

    fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let i = args.iter().position(|a| a == flag)?;
        args.get(i + 1).map(String::as_str)
    }

    #[test]
    fn the_cut_is_an_input_seek_so_both_modes_start_at_the_in_point() {
        for mode in [ExportMode::Precise, ExportMode::Fast] {
            let args = export_args(&job(mode, false), Path::new("/out/t"));
            let input = args.iter().position(|a| a == "-i").unwrap();
            let ss = args.iter().position(|a| a == "-ss").unwrap();
            assert!(ss < input, "{mode:?}: -ss before -i");
            assert_eq!(after(&args, "-ss"), Some("1.000"));
            assert_eq!(after(&args, "-to"), Some("3.500"));
            assert_eq!(args.last().map(String::as_str), Some("/out/t"));
        }
    }

    #[test]
    fn precise_encodes_h264_fast_copies_and_remove_audio_drops_the_track() {
        let precise = export_args(&job(ExportMode::Precise, false), Path::new("/t"));
        assert_eq!(after(&precise, "-c:v"), Some("libx264"));
        assert!(precise.iter().any(|a| a == "0:a?"));
        let fast = export_args(&job(ExportMode::Fast, false), Path::new("/t"));
        assert_eq!(after(&fast, "-c"), Some("copy"));
        assert!(!fast.iter().any(|a| a == "libx264"));
        let silent = export_args(&job(ExportMode::Fast, true), Path::new("/t"));
        assert!(silent.iter().any(|a| a == "-an"));
        assert!(!silent.iter().any(|a| a == "0:a?"));
    }

    #[test]
    fn progress_reads_out_time_and_ignores_the_rest() {
        assert_eq!(progress_secs("out_time_us=1500000"), Some(1.5));
        assert_eq!(progress_secs("out_time_us=N/A"), None);
        assert_eq!(progress_secs("frame=12"), None);
    }

    #[test]
    fn ffmpeg_runs_without_the_display_or_session_bus() {
        let cmd = command("ffmpeg", &[]);
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        for var in ["WAYLAND_DISPLAY", "DISPLAY", "DBUS_SESSION_BUS_ADDRESS"] {
            assert!(removed.iter().any(|r| r == var), "{var} is stripped");
        }
    }

    struct Failing;
    impl FfmpegRunner for Failing {
        fn run(
            &self,
            args: &[String],
            on_line: &mut dyn FnMut(&str),
        ) -> std::io::Result<RunOutcome> {
            on_line("out_time_us=1250000");
            if let Some(target) = args.last() {
                std::fs::write(target, b"half")?;
            }
            Ok(RunOutcome {
                success: false,
                stderr: "noise\nDisk full\n".into(),
            })
        }
    }

    #[test]
    fn a_failed_export_reports_ffmpegs_last_line_and_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut j = job(ExportMode::Precise, false);
        j.output = dir.path().join("clip.mp4");
        let mut seen = Vec::new();
        let err = export(&Failing, &j, &mut |p| seen.push(p));
        assert!(matches!(err, Err(ExportError::Ffmpeg(ref m)) if m == "Disk full"));
        assert_eq!(seen, [0.5]);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
