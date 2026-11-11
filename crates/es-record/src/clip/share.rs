use super::export::{ExportError, ExportJob, FfmpegRunner, run_export};
use super::filters::picture_filters;
use super::options::{ExportOptions, Format, Size};
use std::path::{Path, PathBuf};

pub const AUDIO_KBPS: u32 = 128;
pub const MAX_VIDEO_KBPS: u32 = 8000;
pub const MIN_KBPS_1080: u32 = 600;
pub const MIN_KBPS_720: u32 = 300;
pub const FPS: u32 = 30;
pub const POSTER_MAX_WIDTH: u32 = 1280;
pub const POSTER_MAX_BYTES: u64 = 400 * 1024;
const MARGIN: f64 = 0.95;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharePlan {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub video_kbps: u32,
    pub audio_kbps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("a clip this long does not fit; at most {max_duration_ms_at_720p} ms does")]
pub struct TooLong {
    pub max_duration_ms_at_720p: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("the result is {bytes} bytes, over the {max_bytes} byte limit")]
    OverCap { bytes: u64, max_bytes: u64 },
    #[error("no poster frame fits {POSTER_MAX_BYTES} bytes")]
    PosterTooBig,
}

fn even_fit(w: u32, h: u32, box_w: u32, box_h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let scale = (f64::from(box_w) / f64::from(w))
        .min(f64::from(box_h) / f64::from(h))
        .min(1.);
    let even = |v: u32| ((f64::from(v) * scale / 2.).floor() as u32 * 2).max(2);
    (even(w), even(h))
}

fn budget_bits(max_bytes: u64) -> f64 {
    max_bytes as f64 * 8. * MARGIN
}

pub fn share_plan(
    duration_ms: u64,
    source_w: u32,
    source_h: u32,
    has_audio: bool,
    max_bytes: u64,
) -> Result<SharePlan, TooLong> {
    let audio_kbps = if has_audio { AUDIO_KBPS } else { 0 };
    let secs = duration_ms.max(1) as f64 / 1000.;
    let fitted = (budget_bits(max_bytes) / secs / 1000. - f64::from(audio_kbps)).floor();
    let plan = |box_w, box_h| {
        let (width, height) = even_fit(source_w, source_h, box_w, box_h);
        SharePlan {
            width,
            height,
            fps: FPS,
            video_kbps: fitted.clamp(0., f64::from(MAX_VIDEO_KBPS)) as u32,
            audio_kbps,
        }
    };
    if fitted >= f64::from(MIN_KBPS_1080) {
        return Ok(plan(1920, 1080));
    }
    if fitted >= f64::from(MIN_KBPS_720) {
        return Ok(plan(1280, 720));
    }
    let kbps = f64::from(MIN_KBPS_720 + audio_kbps);
    Err(TooLong {
        max_duration_ms_at_720p: (budget_bits(max_bytes) / kbps) as u64,
    })
}

impl SharePlan {
    pub(super) fn filter(&self, o: &ExportOptions) -> String {
        let mut chain = picture_filters(&ExportOptions {
            size: Size::Original,
            ..*o
        });
        chain.push(format!("scale={}:{}", self.width, self.height));
        chain.push(format!("fps={}", self.fps));
        chain.join(",")
    }

    pub(super) fn codec_args(&self) -> Vec<String> {
        let rate = format!("{}k", self.video_kbps);
        let buf = format!("{}k", self.video_kbps);
        let mut args: Vec<String> = [
            "-c:v",
            "libx264",
            "-profile:v",
            "high",
            "-preset",
            "veryfast",
            "-b:v",
            &rate,
            "-maxrate",
            &rate,
            "-bufsize",
            &buf,
            "-pix_fmt",
            "yuv420p",
        ]
        .map(String::from)
        .to_vec();
        if self.audio_kbps > 0 {
            let audio = format!("{}k", self.audio_kbps);
            args.extend(["-c:a", "aac", "-b:a", &audio].map(String::from));
        }
        args.extend(["-movflags", "+faststart", "-f", "mp4"].map(String::from));
        args
    }
}

/// Whatever format and size the job's options say, the result is H.264 MP4 at the plan's size.
pub fn export_share(
    runner: &dyn FfmpegRunner,
    job: &ExportJob,
    plan: &SharePlan,
    max_bytes: u64,
    progress: &mut dyn FnMut(f64),
) -> Result<PathBuf, ShareError> {
    let mut job = job.clone();
    job.options.format = Format::Mp4;
    let output = run_export(runner, &job, Some(plan), progress)?;
    let bytes = std::fs::metadata(&output).map_err(ExportError::from)?.len();
    if bytes > max_bytes {
        let _ = std::fs::remove_file(&output);
        return Err(ShareError::OverCap { bytes, max_bytes });
    }
    Ok(output)
}

const POSTER_QUALITIES: [u32; 8] = [2, 4, 6, 9, 13, 18, 24, 31];

fn poster_args(video: &Path, at_secs: f64, quality: u32, out: &Path) -> Vec<String> {
    [
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-ss".into(),
        format!("{at_secs:.3}"),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale='min({POSTER_MAX_WIDTH},iw)':-2"),
        "-q:v".into(),
        quality.to_string(),
        out.to_string_lossy().into_owned(),
    ]
    .to_vec()
}

/// One frame of the exported file at `at_secs`, as a JPEG within the poster limits.
pub fn export_poster(
    runner: &dyn FfmpegRunner,
    video: &Path,
    at_secs: f64,
    out: &Path,
) -> Result<(), ShareError> {
    for quality in POSTER_QUALITIES {
        let outcome = runner
            .run(&poster_args(video, at_secs, quality, out), &mut |_| {})
            .map_err(ExportError::from)?;
        if !outcome.success || !out.is_file() {
            let _ = std::fs::remove_file(out);
            let reason = outcome.stderr.trim().lines().last().unwrap_or_default();
            return Err(ExportError::Ffmpeg(reason.to_owned()).into());
        }
        let bytes = std::fs::metadata(out).map_err(ExportError::from)?.len();
        if bytes <= POSTER_MAX_BYTES {
            return Ok(());
        }
    }
    let _ = std::fs::remove_file(out);
    Err(ShareError::PosterTooBig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::export::{SystemFfmpeg, build_args};
    use crate::clip::trim::Trim;

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn the_plan_never_exceeds_the_cap() {
        for max in [5 * MIB, 25 * MIB, 200 * MIB] {
            for secs in [1u64, 10, 60, 300, 900, 1800, 3600] {
                for audio in [false, true] {
                    let Ok(p) = share_plan(secs * 1000, 3840, 2160, audio, max) else {
                        continue;
                    };
                    let kbps = u64::from(p.video_kbps + p.audio_kbps);
                    assert!(kbps * 1000 / 8 * secs <= max, "{secs}s {audio} {max}");
                }
            }
        }
    }

    #[test]
    fn a_short_clip_is_capped_so_it_does_not_waste_the_space() {
        let p = share_plan(5_000, 1920, 1080, true, 200 * MIB).unwrap();
        assert_eq!((p.video_kbps, p.audio_kbps), (MAX_VIDEO_KBPS, 128));
        assert_eq!((p.width, p.height), (1920, 1080));
    }

    #[test]
    fn it_drops_to_720p_below_600_kbps_and_gives_up_below_300() {
        let secs = 600;
        let at = |video_kbps: u64| {
            let bytes = (video_kbps * 1000 / 8 * secs) as f64 / MARGIN;
            share_plan(secs * 1000, 1920, 1080, false, bytes as u64 + 8)
        };
        assert_eq!(at(601).unwrap().height, 1080);
        assert_eq!(at(590).unwrap().height, 720);
        assert_eq!(at(300).unwrap().height, 720);
        assert!(at(290).is_err());
    }

    #[test]
    fn too_long_says_how_long_does_fit() {
        let max = 50 * MIB;
        let err = share_plan(3_600_000, 1920, 1080, true, max).unwrap_err();
        let at_limit = share_plan(err.max_duration_ms_at_720p, 1920, 1080, true, max).unwrap();
        assert!(at_limit.video_kbps >= MIN_KBPS_720 - 1);
        assert!(share_plan(err.max_duration_ms_at_720p + 2000, 1920, 1080, true, max).is_err());
    }

    #[test]
    fn odd_sources_give_even_sizes_and_nothing_is_upscaled() {
        for (w, h) in [
            (1921, 1081),
            (1279, 719),
            (333, 777),
            (3, 3),
            (1, 1),
            (2561, 1441),
        ] {
            for max in [200 * MIB, 20 * MIB] {
                let p = share_plan(120_000, w, h, true, max).unwrap();
                assert!(
                    p.width.is_multiple_of(2) && p.height.is_multiple_of(2),
                    "{w}x{h}"
                );
                assert!(p.width <= w.max(2) && p.height <= h.max(2), "{w}x{h}");
            }
        }
        let small = share_plan(10_000, 640, 360, false, 200 * MIB).unwrap();
        assert_eq!((small.width, small.height), (640, 360));
    }

    #[test]
    fn a_portrait_source_fits_the_box_by_its_height() {
        let p = share_plan(10_000, 1080, 2400, false, 200 * MIB).unwrap();
        assert_eq!((p.width, p.height), (486, 1080));
    }

    fn job(audio_tracks: usize) -> ExportJob {
        ExportJob {
            input: "/v/in.mkv".into(),
            output: "/o/out.mp4".into(),
            trim: Trim::new(3.),
            mode: Default::default(),
            remove_audio: false,
            audio_tracks,
            options: ExportOptions::default(),
        }
    }

    #[test]
    fn the_arguments_cap_the_rate_and_move_the_index_to_the_front() {
        let plan = share_plan(3_000, 1920, 1080, true, 2 * MIB).unwrap();
        let args = build_args(&job(1), Path::new("/t"), Some(&plan));
        let after = |f: &str| {
            let i = args.iter().position(|a| a == f).unwrap();
            args[i + 1].clone()
        };
        assert_eq!(after("-movflags"), "+faststart");
        assert_eq!(after("-maxrate"), format!("{}k", plan.video_kbps));
        assert_eq!(after("-bufsize"), format!("{}k", plan.video_kbps));
        assert_eq!(after("-profile:v"), "high");
        assert_eq!(after("-pix_fmt"), "yuv420p");
        assert!(after("-vf").contains("fps=30"));
    }

    #[test]
    fn a_clip_without_sound_gets_no_audio_stream() {
        let plan = share_plan(3_000, 1920, 1080, false, 2 * MIB).unwrap();
        let args = build_args(&job(0), Path::new("/t"), Some(&plan));
        assert!(args.iter().any(|a| a == "-an") && !args.iter().any(|a| a == "-c:a"));
    }

    #[test]
    fn the_looks_the_user_set_still_apply() {
        let plan = share_plan(3_000, 1920, 1080, false, 2 * MIB).unwrap();
        let mut j = job(0);
        j.options.brightness = 0.1;
        let args = build_args(&j, Path::new("/t"), Some(&plan));
        let i = args.iter().position(|a| a == "-vf").unwrap();
        assert!(args[i + 1].starts_with("eq=brightness=0.100,scale="));
    }

    fn have_x264() -> bool {
        std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-encoders"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("libx264"))
    }

    fn generate(path: &Path, size: &str, secs: u32, audio: bool) {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.args(["-v", "error", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={size}:rate=30:duration={secs}"));
        if audio {
            cmd.args(["-f", "lavfi", "-i"])
                .arg(format!("sine=frequency=440:duration={secs}"))
                .args(["-c:a", "aac"]);
        }
        let made = cmd.args(["-c:v", "libx264", "-y"]).arg(path).status();
        assert!(made.is_ok_and(|s| s.success()));
    }

    #[test]
    fn a_generated_clip_comes_out_under_a_small_cap_with_a_small_poster() {
        if !have_x264() {
            eprintln!("SKIPPED: ffmpeg with libx264 is not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.mkv");
        generate(&input, "1280x720", 3, true);
        let max = 300 * 1024;
        let plan = share_plan(3_000, 1280, 720, true, max).unwrap();
        let mut j = job(1);
        j.input = input;
        j.output = dir.path().join("share.mp4");
        let out = export_share(&SystemFfmpeg, &j, &plan, max, &mut |_| {}).unwrap();
        assert!(std::fs::metadata(&out).unwrap().len() <= max);
        let poster = dir.path().join("poster.jpg");
        export_poster(&SystemFfmpeg, &out, 1.5, &poster).unwrap();
        assert!(std::fs::metadata(&poster).unwrap().len() <= POSTER_MAX_BYTES);
    }

    #[test]
    fn a_result_over_the_cap_is_an_error_and_leaves_no_file() {
        if !have_x264() {
            eprintln!("SKIPPED: ffmpeg with libx264 is not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.mkv");
        generate(&input, "640x360", 2, false);
        let plan = share_plan(2_000, 640, 360, false, 10 * MIB).unwrap();
        let mut j = job(0);
        j.input = input;
        j.output = dir.path().join("share.mp4");
        let err = export_share(&SystemFfmpeg, &j, &plan, 1000, &mut |_| {}).unwrap_err();
        assert!(matches!(err, ShareError::OverCap { .. }));
        assert!(!j.output.exists());
    }
}
