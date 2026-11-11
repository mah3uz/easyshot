#![allow(clippy::unwrap_used)]
use es_app::share::{Caps, PrepareError, PrepareJob, Tools, prepare_with};
use es_model::ShareKind;
use es_record::clip::export::{ExportJob, ExportMode, FfmpegRunner, RunOutcome};
use es_record::clip::options::ExportOptions;
use es_record::clip::trim::Trim;
use es_record::project::CommandRunner;
use es_store::Paths;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const KB: u64 = 1024;

struct Probe {
    duration: f64,
}

impl CommandRunner for Probe {
    fn run(&self, _program: &str, args: &[String]) -> std::io::Result<Output> {
        let stdout = if args.iter().any(|a| a == "json") {
            format!(
                r#"{{"streams":[{{"width":1920,"height":1080,"codec_name":"h264"}}],"format":{{"duration":"{}"}}}}"#,
                self.duration
            )
        } else {
            "1\n".to_owned()
        };
        Ok(Output {
            status: ExitStatus::from_raw(0),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        })
    }
}

#[derive(Default)]
struct CountedFfmpeg(AtomicUsize);

impl FfmpegRunner for CountedFfmpeg {
    fn run(&self, _: &[String], _: &mut dyn FnMut(&str)) -> std::io::Result<RunOutcome> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::other("not run in tests"))
    }
}

struct Rig {
    _dir: tempfile::TempDir,
    paths: Paths,
    cancel: AtomicBool,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    Rig {
        _dir: dir,
        paths,
        cancel: AtomicBool::new(false),
    }
}

fn job(rig: &Rig, image_max_bytes: u64, video_max_bytes: u64) -> PrepareJob<'_> {
    PrepareJob {
        paths: &rig.paths,
        caps: Caps {
            image_max_bytes,
            video_max_bytes,
        },
        video: ExportOptions::default(),
        export: None,
        cancel: &rig.cancel,
    }
}

fn noisy_image(dir: &Path, name: &str, side: u32) -> PathBuf {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let img = image::RgbaImage::from_fn(side, side, |_, _| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = (state >> 33) as u32;
        image::Rgba([v as u8, (v >> 8) as u8, (v >> 16) as u8, 255])
    });
    let file = dir.join(name);
    img.save(&file).unwrap();
    file
}

fn tools<'a>(probe: &'a Probe, ffmpeg: &'a CountedFfmpeg) -> Tools<'a> {
    Tools { probe, ffmpeg }
}

#[test]
fn an_image_that_fits_is_sent_as_it_is_with_no_temp_files() {
    let rig = rig();
    let file = noisy_image(rig._dir.path(), "a.png", 64);
    let size = std::fs::metadata(&file).unwrap().len();
    let (probe, ffmpeg) = (Probe { duration: 1. }, CountedFfmpeg::default());
    let prepared = prepare_with(
        &file,
        &job(&rig, size, 0),
        &tools(&probe, &ffmpeg),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(prepared.media.path, file);
    assert_eq!(prepared.media.mime, "image/png");
    assert_eq!(prepared.media.kind, ShareKind::Image);
    assert_eq!((prepared.media.width, prepared.media.height), (64, 64));
    assert_eq!(std::fs::read_dir(rig.paths.share_dir()).unwrap().count(), 0);
}

#[test]
fn an_image_over_the_cap_is_exported_once_under_it_and_its_files_go_with_it() {
    let rig = rig();
    let file = noisy_image(rig._dir.path(), "big.png", 700);
    let cap = 60 * KB;
    assert!(std::fs::metadata(&file).unwrap().len() > cap * 4);
    let (probe, ffmpeg) = (Probe { duration: 1. }, CountedFfmpeg::default());
    let prepared = prepare_with(
        &file,
        &job(&rig, cap, 0),
        &tools(&probe, &ffmpeg),
        &mut |_| {},
    )
    .unwrap();
    let out = prepared.media.path.clone();
    assert_eq!(prepared.media.mime, "image/jpeg");
    assert!(std::fs::metadata(&out).unwrap().len() <= cap);
    assert!(prepared.media.width < 700 && prepared.media.width >= 32);
    let (w, h) = image::image_dimensions(&out).unwrap();
    assert_eq!((w, h), (prepared.media.width, prepared.media.height));
    assert!(out.starts_with(rig.paths.share_dir()));
    drop(prepared);
    assert!(!out.exists(), "the temp files end with the job");
}

#[test]
fn an_image_that_cannot_shrink_under_the_cap_is_too_large() {
    let rig = rig();
    let file = noisy_image(rig._dir.path(), "big.png", 200);
    let (probe, ffmpeg) = (Probe { duration: 1. }, CountedFfmpeg::default());
    let err = prepare_with(
        &file,
        &job(&rig, 100, 0),
        &tools(&probe, &ffmpeg),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err, PrepareError::TooLarge);
    assert_eq!(std::fs::read_dir(rig.paths.share_dir()).unwrap().count(), 0);
}

#[test]
fn a_clip_too_long_for_the_cap_says_how_long_fits_and_encodes_nothing() {
    let rig = rig();
    let video = rig._dir.path().join("long.mp4");
    std::fs::write(&video, b"x").unwrap();
    let (probe, ffmpeg) = (Probe { duration: 3600. }, CountedFfmpeg::default());
    let cap = 50 * 1024 * 1024;
    let err = prepare_with(
        &video,
        &job(&rig, 0, cap),
        &tools(&probe, &ffmpeg),
        &mut |_| {},
    )
    .unwrap_err();
    let PrepareError::TooLong {
        duration_ms,
        max_duration_ms_at_720p,
    } = err
    else {
        panic!("{err:?}");
    };
    assert_eq!(duration_ms, 3_600_000);
    assert!(
        (900_000..1_000_000).contains(&max_duration_ms_at_720p),
        "{max_duration_ms_at_720p}"
    );
    assert_eq!(ffmpeg.0.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read_dir(rig.paths.share_dir()).unwrap().count(), 0);
}

#[test]
fn a_cancelled_job_makes_nothing() {
    let rig = rig();
    rig.cancel.store(true, Ordering::Relaxed);
    let file = noisy_image(rig._dir.path(), "big.png", 400);
    let (probe, ffmpeg) = (Probe { duration: 1. }, CountedFfmpeg::default());
    let err = prepare_with(
        &file,
        &job(&rig, 10 * KB, 0),
        &tools(&probe, &ffmpeg),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(err, PrepareError::Cancelled);
    assert_eq!(std::fs::read_dir(rig.paths.share_dir()).unwrap().count(), 0);
}

#[derive(Default)]
struct RecordedFfmpeg(Mutex<Vec<Vec<String>>>);

impl FfmpegRunner for RecordedFfmpeg {
    fn run(&self, args: &[String], _: &mut dyn FnMut(&str)) -> std::io::Result<RunOutcome> {
        self.0.lock().unwrap().push(args.to_vec());
        Err(std::io::Error::other("not run in tests"))
    }
}

fn after(args: &[String], flag: &str) -> String {
    let i = args.iter().position(|a| a == flag).unwrap();
    args[i + 1].clone()
}

#[test]
fn a_clip_trimmed_in_the_viewer_is_encoded_once_as_trimmed_with_its_look() {
    let rig = rig();
    let video = rig._dir.path().join("clip.mp4");
    std::fs::write(&video, b"x").unwrap();
    let mut trim = Trim::new(60.);
    trim.set_start(12.);
    trim.set_end(20.5);
    let export = ExportJob {
        input: video.clone(),
        output: PathBuf::new(),
        trim,
        mode: ExportMode::Fast,
        remove_audio: false,
        audio_tracks: 1,
        options: ExportOptions {
            brightness: 0.2,
            ..ExportOptions::default()
        },
    };
    let ffmpeg = RecordedFfmpeg::default();
    let probe = Probe { duration: 60. };
    let mut job = job(&rig, 0, 1024 * 1024 * 1024);
    job.export = Some(&export);
    let tools = Tools {
        probe: &probe,
        ffmpeg: &ffmpeg,
    };
    let _ = prepare_with(&video, &job, &tools, &mut |_| {});
    let runs = ffmpeg.0.lock().unwrap();
    assert_eq!(runs.len(), 1, "one encode, no export first: {runs:?}");
    let args = &runs[0];
    assert_eq!(after(args, "-ss"), "12.000");
    assert_eq!(after(args, "-to"), "20.500");
    assert!(
        after(args, "-vf").starts_with("eq=brightness=0.200,"),
        "{args:?}"
    );
    assert!(after(args, "-maxrate").ends_with('k'), "{args:?}");
}

#[test]
fn a_clip_trimmed_to_fit_is_not_too_long_though_the_recording_is() {
    let rig = rig();
    let video = rig._dir.path().join("long.mp4");
    std::fs::write(&video, b"x").unwrap();
    let mut trim = Trim::new(3600.);
    trim.set_end(30.);
    let export = ExportJob {
        input: video.clone(),
        output: PathBuf::new(),
        trim,
        mode: ExportMode::Precise,
        remove_audio: false,
        audio_tracks: 1,
        options: ExportOptions::default(),
    };
    let ffmpeg = RecordedFfmpeg::default();
    let probe = Probe { duration: 3600. };
    let mut job = job(&rig, 0, 50 * 1024 * 1024);
    job.export = Some(&export);
    let tools = Tools {
        probe: &probe,
        ffmpeg: &ffmpeg,
    };
    let err = prepare_with(&video, &job, &tools, &mut |_| {}).unwrap_err();
    assert!(!matches!(err, PrepareError::TooLong { .. }), "{err:?}");
    assert_eq!(
        ffmpeg.0.lock().unwrap().len(),
        1,
        "the 30 s kept is encoded"
    );
}
