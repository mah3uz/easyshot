use super::*;
use es_protocol::agent::TrimRange;
use es_record::clip::export::export_args;
use std::process::Command;

fn make_video(dir: &Path, secs: u32) -> PathBuf {
    let path = dir.join("in.mp4");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg(format!("testsrc=size=1280x720:rate=10:duration={secs}"))
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=duration={secs}"))
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
        .arg(&path)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .status()
        .unwrap();
    assert!(status.success());
    path
}

fn spec(input: &Path) -> ExportSpec {
    ExportSpec::new(input.to_path_buf())
}

fn go(spec: &ExportSpec) -> Result<ExportInfo, Fail> {
    run(spec, &SystemRunner, &SystemFfmpeg)
}

fn refused(spec: &ExportSpec) -> ErrorCode {
    match go(spec) {
        Err(Fail(code, ..)) => code,
        Ok(done) => panic!("exported {}", done.path),
    }
}

#[test]
fn every_format_exports_at_the_size_and_trim_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let input = make_video(dir.path(), 4);
    for format in ["mp4", "webm", "gif"] {
        let out = dir.path().join(format!("clip.{format}"));
        let done = go(&ExportSpec {
            format: Some(format.into()),
            size: Some("480p".into()),
            trim: TrimRange::parse("1..3").unwrap(),
            out: Some(out.clone()),
            ..spec(&input)
        })
        .unwrap_or_else(|Fail(c, m, ..)| panic!("{format}: {c:?} {m}"));
        assert_eq!(done.path, out.display().to_string());
        assert_eq!(done.format, format);
        assert_eq!((done.width, done.height), (854, 480), "{format}");
        assert!(
            (1800..=2300).contains(&done.duration_ms),
            "{format}: {} ms",
            done.duration_ms
        );
        assert_eq!(done.bytes, std::fs::metadata(&out).unwrap().len());
        assert!(done.bytes > 0);
    }
}

#[test]
fn an_open_ended_trim_runs_to_the_end_and_the_default_file_sits_beside_the_video() {
    let dir = tempfile::tempdir().unwrap();
    let input = make_video(dir.path(), 4);
    let done = go(&ExportSpec {
        trim: TrimRange::parse("2..").unwrap(),
        ..spec(&input)
    })
    .unwrap();
    assert_eq!(
        done.path,
        dir.path().join("in-export.mp4").display().to_string()
    );
    assert_eq!((done.width, done.height), (1280, 720));
    assert!(
        (1800..=2300).contains(&done.duration_ms),
        "{}",
        done.duration_ms
    );
}

#[test]
fn the_options_become_the_same_ffmpeg_arguments_the_trim_viewer_makes() {
    let dir = tempfile::tempdir().unwrap();
    let job = job(
        &ExportSpec {
            format: Some("webm".into()),
            size: Some("720p".into()),
            quality: Some("high".into()),
            look: Some("mono".into()),
            brightness_milli: -100,
            trim: TrimRange::parse("1.5..3").unwrap(),
            ..spec(&dir.path().join("in.mp4"))
        },
        10.,
    )
    .unwrap_or_else(|Fail(c, m, ..)| panic!("{c:?} {m}"));
    assert_eq!(
        job.options,
        ExportOptions {
            format: Format::WebM,
            size: Size::P720,
            quality: Quality::High,
            look: Look::Mono,
            brightness: -0.1,
            system_volume: 0.5,
            mic_volume: 1.,
        }
    );
    assert_eq!((job.trim.start(), job.trim.end()), (1.5, 3.));
    let args = export_args(&job, Path::new("/t")).join(" ");
    assert!(args.contains("-ss 1.500 -to 3.000"), "{args}");
    assert!(
        args.contains("libvpx-vp9") && args.contains("-crf 28"),
        "{args}"
    );
    assert!(
        args.contains(
            "scale=-2:'min(720,ih)',eq=brightness=-0.100:contrast=1.050:saturation=0.000"
        ),
        "{args}"
    );
}

#[test]
fn a_trim_is_held_to_the_length_of_the_video() {
    let dir = tempfile::tempdir().unwrap();
    let make = |range: &str| {
        job(
            &ExportSpec {
                trim: TrimRange::parse(range).unwrap(),
                ..spec(&dir.path().join("in.mp4"))
            },
            4.,
        )
    };
    assert_eq!(make("1..99").map(|j| j.trim.end()).ok(), Some(4.));
    for past in ["4..", "5..9", "3.95.."] {
        assert!(
            matches!(make(past), Err(Fail(ErrorCode::InvalidArguments, ..))),
            "{past}"
        );
    }
}

struct NoTools;

impl CommandRunner for NoTools {
    fn run(&self, _: &str, _: &[String]) -> std::io::Result<std::process::Output> {
        Err(std::io::ErrorKind::NotFound.into())
    }
}

#[test]
fn without_ffmpeg_the_reply_says_what_to_install() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.mp4");
    std::fs::write(&input, "x").unwrap();
    let Err(Fail(code, _, hint)) = run(&spec(&input), &NoTools, &SystemFfmpeg) else {
        panic!("exported without ffmpeg");
    };
    assert_eq!(code, ErrorCode::Unsupported);
    assert_eq!(
        hint.as_deref(),
        Some("Exporting needs ffmpeg. Install it with `sudo pacman -S ffmpeg`.")
    );
}

#[test]
fn a_missing_video_hint_names_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.mp4");
    let Err(Fail(_, _, hint)) = run(&spec(&gone), &NoTools, &SystemFfmpeg) else {
        panic!("exported a missing file");
    };
    assert_eq!(
        hint,
        Some(format!(
            "There is no file at {}. Check the path and try again.",
            gone.display()
        ))
    );
}

#[test]
fn a_missing_file_is_not_found_and_other_files_are_not_videos() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        refused(&spec(&dir.path().join("gone.mp4"))),
        ErrorCode::NotFound
    );
    let text = dir.path().join("notes.mp4");
    std::fs::write(&text, "not a video").unwrap();
    assert_eq!(refused(&spec(&text)), ErrorCode::InvalidArguments);
    assert_eq!(refused(&spec(dir.path())), ErrorCode::InvalidArguments);
}

#[test]
fn a_file_is_replaced_only_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let input = make_video(dir.path(), 2);
    let out = dir.path().join("clip.mp4");
    std::fs::write(&out, "precious").unwrap();
    let ask = |force| ExportSpec {
        out: Some(out.clone()),
        force,
        ..spec(&input)
    };
    assert_eq!(refused(&ask(false)), ErrorCode::InvalidArguments);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "precious");
    go(&ask(true)).unwrap();
    assert!(std::fs::metadata(&out).unwrap().len() > 100);
}

#[test]
fn the_output_folder_must_exist_and_the_input_is_never_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = make_video(dir.path(), 2);
    let into = |out: PathBuf| ExportSpec {
        out: Some(out),
        force: true,
        ..spec(&input)
    };
    assert_eq!(
        refused(&into(dir.path().join("no/such/clip.mp4"))),
        ErrorCode::InvalidArguments
    );
    assert_eq!(refused(&into(input.clone())), ErrorCode::InvalidArguments);
    assert!(std::fs::metadata(&input).unwrap().len() > 100);
}

fn audio_tracks(file: &Path) -> usize {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "a", "-show_entries"])
        .args(["stream=index", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).lines().count()
}

#[test]
fn a_recording_with_system_and_mic_tracks_exports_with_one_mixed_track() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("two.mkv");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i"])
        .arg("testsrc=size=320x240:rate=10:duration=2")
        .args(["-f", "lavfi", "-i", "sine=frequency=300:duration=2"])
        .args(["-f", "lavfi", "-i", "sine=frequency=900:duration=2"])
        .args(["-map", "0:v", "-map", "1:a", "-map", "2:a"])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "libopus"])
        .arg(&input)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(audio_tracks(&input), 2);
    for format in ["mp4", "webm"] {
        let out = dir.path().join(format!("clip.{format}"));
        go(&ExportSpec {
            format: Some(format.into()),
            out: Some(out.clone()),
            ..spec(&input)
        })
        .unwrap_or_else(|Fail(c, m, ..)| panic!("{format}: {c:?} {m}"));
        assert_eq!(audio_tracks(&out), 1, "{format} mixes the tracks into one");
    }
}
