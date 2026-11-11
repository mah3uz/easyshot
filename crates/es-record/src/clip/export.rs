use super::filters::{codec_args, gif_filter, picture_filters};
use super::mix::{audio_codec_args, mix_filter, mixes};
use super::options::{ExportOptions, Format};
use super::share::SharePlan;
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
    /// How many sound tracks the input has; two are mixed into one by the volumes in `options`.
    pub audio_tracks: usize,
    pub options: ExportOptions,
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

/// A stream copy only keeps the picture as it is, so any size, tone or format change re-encodes.
fn copies_picture(job: &ExportJob) -> bool {
    job.mode == ExportMode::Fast
        && job.options.format == Format::Mp4
        && !job.options.alters_picture()
}

pub fn export_args(job: &ExportJob, target: &Path) -> Vec<String> {
    build_args(job, target, None)
}

pub(super) fn build_args(job: &ExportJob, target: &Path, share: Option<&SharePlan>) -> Vec<String> {
    let o = &job.options;
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
    if o.format == Format::Gif || job.remove_audio || share.is_some_and(|p| p.audio_kbps == 0) {
        args.push("-an".into());
    } else if mixes(job) {
        args.extend(["-filter_complex".into(), mix_filter(o)]);
        args.extend(["-map", "[mix]"].map(String::from));
    } else {
        args.extend(["-map", "0:a?"].map(String::from));
    }
    if let Some(plan) = share {
        args.extend(["-vf".into(), plan.filter(o)]);
        args.extend(plan.codec_args());
    } else if copies_picture(job) {
        if mixes(job) {
            args.extend(["-c:v", "copy"].map(String::from));
            args.extend(audio_codec_args(o));
        } else {
            args.extend(["-c", "copy"].map(String::from));
        }
        args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
    } else {
        if o.format == Format::Gif {
            args.extend(["-vf".into(), gif_filter(o)]);
        } else {
            let filters = picture_filters(o);
            if !filters.is_empty() {
                args.extend(["-vf".into(), filters.join(",")]);
            }
        }
        args.extend(codec_args(o));
    }
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
    run_export(runner, job, None, progress)
}

pub(super) fn run_export(
    runner: &dyn FfmpegRunner,
    job: &ExportJob,
    share: Option<&SharePlan>,
    progress: &mut dyn FnMut(f64),
) -> Result<PathBuf, ExportError> {
    let target = partial(&job.output);
    let kept = job.trim.kept().max(f64::EPSILON);
    let outcome = runner.run(&build_args(job, &target, share), &mut |line| {
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
    use super::super::options::{Look, Quality, Size};
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
            audio_tracks: 1,
            options: ExportOptions::default(),
        }
    }

    fn two_tracks(options: ExportOptions, mode: ExportMode) -> ExportJob {
        ExportJob {
            audio_tracks: 2,
            options,
            ..job(mode, false)
        }
    }

    fn with(options: ExportOptions) -> ExportJob {
        ExportJob {
            options,
            ..job(ExportMode::Precise, false)
        }
    }

    fn args_for(options: ExportOptions) -> Vec<String> {
        export_args(&with(options), Path::new("/t"))
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
    fn one_track_is_mapped_as_it_is_and_never_mixed() {
        let args = export_args(&job(ExportMode::Precise, false), Path::new("/t"));
        assert!(!args.iter().any(|a| a == "-filter_complex"));
        assert!(args.iter().any(|a| a == "0:a?"));
    }

    #[test]
    fn two_tracks_are_mixed_into_one_at_the_chosen_volumes() {
        let options = ExportOptions {
            system_volume: 0.5,
            mic_volume: 1.5,
            ..Default::default()
        };
        let args = export_args(&two_tracks(options, ExportMode::Precise), Path::new("/t"));
        let graph = after(&args, "-filter_complex").unwrap();
        assert!(graph.contains("[0:a:0]volume=0.500[sys]"), "{graph}");
        assert!(graph.contains("[0:a:1]volume=1.500[mic]"), "{graph}");
        assert!(graph.contains("amix=inputs=2:normalize=0"), "{graph}");
        assert!(args.windows(2).any(|w| w == ["-map", "[mix]"]));
        assert!(!args.iter().any(|a| a == "0:a?"));
        assert_eq!(after(&args, "-c:a"), Some("aac"));
    }

    #[test]
    fn a_fast_export_of_two_tracks_copies_the_picture_and_encodes_the_mix() {
        let args = export_args(
            &two_tracks(ExportOptions::default(), ExportMode::Fast),
            Path::new("/t"),
        );
        assert_eq!(after(&args, "-c:v"), Some("copy"));
        assert_eq!(after(&args, "-c:a"), Some("aac"));
        assert!(after(&args, "-filter_complex").is_some());
    }

    #[test]
    fn a_gif_or_removed_audio_never_mixes() {
        let gif = ExportOptions {
            format: Format::Gif,
            ..Default::default()
        };
        let args = export_args(&two_tracks(gif, ExportMode::Precise), Path::new("/t"));
        assert!(!args.iter().any(|a| a == "-filter_complex"));
        let mut muted = two_tracks(ExportOptions::default(), ExportMode::Precise);
        muted.remove_audio = true;
        let args = export_args(&muted, Path::new("/t"));
        assert!(args.iter().any(|a| a == "-an") && !args.iter().any(|a| a == "-filter_complex"));
    }

    #[test]
    fn each_format_gets_its_codecs_and_container() {
        let mp4 = args_for(ExportOptions::default());
        assert_eq!(after(&mp4, "-c:v"), Some("libx264"));
        assert_eq!(after(&mp4, "-c:a"), Some("aac"));
        assert_eq!(after(&mp4, "-f"), Some("mp4"));
        let webm = args_for(ExportOptions {
            format: Format::WebM,
            ..Default::default()
        });
        assert_eq!(after(&webm, "-c:v"), Some("libvpx-vp9"));
        assert_eq!(after(&webm, "-c:a"), Some("libopus"));
        assert_eq!(after(&webm, "-f"), Some("webm"));
        let gif = args_for(ExportOptions {
            format: Format::Gif,
            ..Default::default()
        });
        assert_eq!(after(&gif, "-f"), Some("gif"));
        assert!(!gif.iter().any(|a| a == "-c:v" || a == "-c:a"));
    }

    #[test]
    fn a_gif_has_no_audio_even_when_audio_is_kept() {
        let gif = args_for(ExportOptions {
            format: Format::Gif,
            ..Default::default()
        });
        assert!(gif.iter().any(|a| a == "-an"));
        assert!(!gif.iter().any(|a| a == "0:a?"));
        let vf = after(&gif, "-vf").unwrap();
        assert!(vf.contains("palettegen") && vf.contains("paletteuse"));
    }

    #[test]
    fn quality_lowers_the_bitrate_in_every_video_format() {
        for format in [Format::Mp4, Format::WebM] {
            let crf = |quality| {
                let args = args_for(ExportOptions {
                    format,
                    quality,
                    ..Default::default()
                });
                after(&args, "-crf").unwrap().parse::<u32>().unwrap()
            };
            let (high, balanced, small) = (
                crf(Quality::High),
                crf(Quality::Balanced),
                crf(Quality::Small),
            );
            assert!(high < balanced && balanced < small, "{format:?}");
        }
        let fps = |quality| {
            let args = args_for(ExportOptions {
                format: Format::Gif,
                quality,
                ..Default::default()
            });
            after(&args, "-vf").unwrap().to_owned()
        };
        assert_ne!(fps(Quality::High), fps(Quality::Small));
    }

    #[test]
    fn a_size_only_ever_scales_down_and_keeps_the_aspect() {
        assert!(after(&args_for(ExportOptions::default()), "-vf").is_none());
        for (size, h) in [(Size::P1080, 1080), (Size::P720, 720), (Size::P480, 480)] {
            let args = args_for(ExportOptions {
                size,
                ..Default::default()
            });
            assert_eq!(
                after(&args, "-vf"),
                Some(format!("scale=-2:'min({h},ih)'").as_str())
            );
        }
    }

    #[test]
    fn brightness_and_looks_become_one_eq_filter() {
        let vf = |look, brightness| {
            let args = args_for(ExportOptions {
                look,
                brightness,
                ..Default::default()
            });
            after(&args, "-vf").map(str::to_owned)
        };
        assert_eq!(vf(Look::Natural, 0.), None);
        assert_eq!(
            vf(Look::Natural, 0.1).as_deref(),
            Some("eq=brightness=0.100")
        );
        assert_eq!(
            vf(Look::Mono, 0.).as_deref(),
            Some("eq=contrast=1.050:saturation=0.000")
        );
        let vivid = vf(Look::Vivid, -0.1).unwrap();
        assert!(vivid.contains("brightness=-0.100") && vivid.contains("saturation=1.400"));
    }

    #[test]
    fn a_stream_copy_is_only_for_an_untouched_mp4() {
        let fast = |options| {
            let job = ExportJob {
                mode: ExportMode::Fast,
                options,
                ..job(ExportMode::Fast, false)
            };
            export_args(&job, Path::new("/t"))
        };
        assert_eq!(after(&fast(ExportOptions::default()), "-c"), Some("copy"));
        for options in [
            ExportOptions {
                format: Format::WebM,
                ..Default::default()
            },
            ExportOptions {
                size: Size::P720,
                ..Default::default()
            },
            ExportOptions {
                brightness: 0.1,
                ..Default::default()
            },
        ] {
            assert_eq!(after(&fast(options), "-c"), None, "{options:?}");
        }
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
