#![cfg(feature = "player")]
#![allow(clippy::unwrap_used)]
use crate::window_mode_support;

use es_overlays::player::{
    AudioOut, ExportJob, ExportMode, ExportOptions, ExportState, Format, HANDLE_WIDTH, Look,
    MIN_TRIM_SECS, PlayerConfig, PlayerEvent, PlayerWindow, Quality, Size, SystemFfmpeg, Trim,
    export::export,
};
use es_record::{CommandRunner, SystemRunner};
use es_store::Paths;
use es_ui::tokens::ThemeMode;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use window_mode_support::Surface;

const FPS: f64 = 30.;
const FRAME: f64 = 1. / FPS;

/// Five seconds of moving colour bars with a tone. A keyframe every half second and no B-frames, as a screen recorder writes, so a stream copy can cut exactly at 1.0 s.
fn test_video(dir: &Path) -> PathBuf {
    sized_video(dir, "clip.mkv", "640x360")
}

fn sized_video(dir: &Path, name: &str, size: &str) -> PathBuf {
    let out = dir.join(name);
    let args: Vec<String> = [
        "-y",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        &format!("testsrc=size={size}:rate=30:duration=5"),
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=5",
        "-c:v",
        "libx264",
        "-g",
        "15",
        "-bf",
        "0",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-shortest",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([out.to_string_lossy().into_owned()])
    .collect();
    let made = SystemRunner.run("ffmpeg", &args).unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    out
}

struct Counting(AtomicUsize);

impl CommandRunner for Counting {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        self.0.fetch_add(1, Ordering::SeqCst);
        SystemRunner.run(program, args)
    }
}

struct Rig {
    s: Surface<PlayerWindow>,
    dir: PathBuf,
}

fn open(dir: &Path, video: &Path, commands: Arc<dyn CommandRunner>) -> Rig {
    let config = PlayerConfig {
        video: video.to_path_buf(),
        output: {
            let dir = dir.to_path_buf();
            std::rc::Rc::new(move |ext| dir.join(format!("trimmed.{ext}")))
        },
        options: ExportOptions::default(),
        audio: AudioOut::Silent,
        paths: Paths::rooted(&dir.join("root")),
        commands,
        ffmpeg: Arc::new(SystemFfmpeg),
    };
    let s = Surface::open(960., 600., move |cx| PlayerWindow::new(config, cx));
    let mut r = Rig {
        s,
        dir: dir.to_path_buf(),
    };
    r.wait("the video to open", |p| {
        p.trim().duration() > 0. && p.frames_shown() > 0
    });
    r
}

impl Rig {
    fn read<R>(&mut self, f: impl FnOnce(&PlayerWindow) -> R) -> R {
        let view = self.s.view.clone();
        self.s.app.cx.update(|cx| f(view.read(cx)))
    }

    fn wait(&mut self, what: &str, done: impl Fn(&PlayerWindow) -> bool) {
        let view = self.s.view.clone();
        self.s.app.until(what, |cx| done(view.read(cx)));
    }

    /// Window x of a time on the strip, and the strip's middle row.
    fn at(&mut self, secs: f64) -> (f32, f32) {
        let (b, d) = self.read(|p| (p.track(), p.trim().duration()));
        let x = f32::from(b.left()) + (secs / d) as f32 * f32::from(b.size.width);
        (x, f32::from(b.center().y))
    }

    fn drag(&mut self, from: (f32, f32), to_x: f32) {
        self.s.hover(from.0, from.1, false);
        self.s.down(from.0, from.1);
        self.s.hover(to_x, from.1, true);
        self.s.up(to_x, from.1);
    }

    fn start_handle(&mut self) -> (f32, f32) {
        let start = self.read(|p| p.trim().start());
        let (x, y) = self.at(start);
        (x - f32::from(HANDLE_WIDTH) / 2., y)
    }

    fn end_handle(&mut self) -> (f32, f32) {
        let end = self.read(|p| p.trim().end());
        let (x, y) = self.at(end);
        (x + f32::from(HANDLE_WIDTH) / 2., y)
    }
}

fn rig() -> (tempfile::TempDir, Rig) {
    let dir = tempfile::tempdir().unwrap();
    let video = test_video(dir.path());
    let r = open(dir.path(), &video, Arc::new(SystemRunner));
    (dir, r)
}

fn probe_duration(file: &Path) -> f64 {
    es_record::recovery::probe_video(&SystemRunner, file)
        .unwrap()
        .duration_secs
}

fn audio_streams(file: &Path) -> String {
    let args = [
        "-v",
        "error",
        "-select_streams",
        "a",
        "-show_entries",
        "stream=index",
        "-of",
        "csv=p=0",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .chain([file.to_string_lossy().into_owned()])
    .collect::<Vec<_>>();
    let out = SystemRunner.run("ffprobe", &args).unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn playing_replaces_each_frame_and_frees_the_one_before() {
    let (_d, mut r) = rig();
    assert!((r.read(|p| p.trim().duration()) - 5.).abs() < 0.1);
    r.s.key("space");
    assert!(r.read(PlayerWindow::is_playing));
    r.wait("a dozen frames", |p| p.frames_shown() >= 12);
    let (shown, dropped) = r.read(|p| (p.frames_shown(), p.images_dropped()));
    assert_eq!(
        dropped,
        shown - 1,
        "every frame but the one on screen left the atlas"
    );
    assert!(r.read(PlayerWindow::position) > 0.2, "playback advanced");

    r.s.key("space");
    assert!(!r.read(PlayerWindow::is_playing));
    std::thread::sleep(Duration::from_millis(100));
    r.s.app.settle();
    let (at, shown) = r.read(|p| (p.position(), p.frames_shown()));
    std::thread::sleep(Duration::from_millis(400));
    r.s.app.settle();
    assert_eq!(
        r.read(PlayerWindow::position),
        at,
        "paused playback stays put"
    );
    assert!(r.read(PlayerWindow::frames_shown) <= shown + 1);
}

#[test]
fn clicking_the_strip_seeks_and_play_continues_from_there() {
    let (_d, mut r) = rig();
    let (x, y) = r.at(3.);
    r.s.click(x, y);
    assert!((r.read(PlayerWindow::position) - 3.).abs() < 0.05);
    r.s.key("space");
    std::thread::sleep(Duration::from_millis(250));
    r.wait("frames after the seek", |p| p.position() > 3.05);
    let at = r.read(PlayerWindow::position);
    assert!(
        (3.05..3.8).contains(&at),
        "playing from the seek point, now {at}"
    );
}

#[test]
fn trim_handles_never_come_closer_than_the_minimum() {
    let (_d, mut r) = rig();
    let left = r.at(0.).0 - 20.;
    let end = r.end_handle();
    r.drag(end, left);
    let t = r.read(PlayerWindow::trim);
    assert_eq!(t.start(), 0.);
    assert!(
        (t.end() - MIN_TRIM_SECS).abs() < 1e-9,
        "end stops at {}",
        t.end()
    );

    let right = r.at(5.).0 + 20.;
    let end = r.end_handle();
    r.drag(end, right);
    let start = r.start_handle();
    r.drag(start, right);
    let t = r.read(PlayerWindow::trim);
    assert_eq!(t.end(), t.duration());
    assert!(
        (t.kept() - MIN_TRIM_SECS).abs() < 1e-9,
        "start stops at {}",
        t.start()
    );
    assert!(
        (r.read(PlayerWindow::position) - t.start()).abs() < 1e-9,
        "the playhead previews the handle"
    );
}

fn trimmed(input: &Path, output: PathBuf, mode: ExportMode, remove_audio: bool) -> ExportJob {
    let mut trim = Trim::new(5.);
    trim.set_start(1.);
    trim.set_end(3.5);
    ExportJob {
        input: input.to_path_buf(),
        output,
        trim,
        mode,
        remove_audio,
        options: ExportOptions::default(),
    }
}

#[test]
fn both_export_methods_keep_the_trimmed_span_and_remove_audio_drops_it() {
    let dir = tempfile::tempdir().unwrap();
    let video = test_video(dir.path());
    for (mode, name) in [
        (ExportMode::Precise, "precise.mp4"),
        (ExportMode::Fast, "fast.mp4"),
    ] {
        let mut seen = Vec::new();
        let job = trimmed(&video, dir.path().join(name), mode, false);
        let out = export(&SystemFfmpeg, &job, &mut |p| seen.push(p)).unwrap();
        let d = probe_duration(&out);
        assert!((d - 2.5).abs() <= FRAME + 1e-3, "{mode:?} lasts {d}");
        assert_eq!(audio_streams(&out), "1", "{mode:?} keeps the audio");
        assert_eq!(seen.last(), Some(&1.));
        let codec = es_record::recovery::probe_video(&SystemRunner, &out)
            .unwrap()
            .codec;
        assert_eq!(codec, "h264");
    }
    for mode in [ExportMode::Precise, ExportMode::Fast] {
        let job = trimmed(&video, dir.path().join("silent.mp4"), mode, true);
        let out = export(&SystemFfmpeg, &job, &mut |_| {}).unwrap();
        assert_eq!(audio_streams(&out), "", "{mode:?} has no audio stream");
    }
}

#[test]
fn the_export_button_writes_the_trim_and_reports_it() {
    let (_d, mut r) = rig();
    let to = r.at(4.).0;
    let end = r.end_handle();
    r.drag(end, to);
    let view = r.s.view.clone();
    r.s.app
        .cx
        .update(|cx| view.update(cx, |p, cx| p.export(cx)));
    assert!(matches!(
        r.read(|p| p.export_state().clone()),
        ExportState::Running(_)
    ));
    r.wait("the export", |p| {
        !matches!(p.export_state(), ExportState::Running(_))
    });
    let out = r.dir.join("trimmed.mp4");
    assert_eq!(
        r.read(|p| p.export_state().clone()),
        ExportState::Done(out.clone())
    );
    let kept = r.read(|p| p.trim().kept());
    assert!((probe_duration(&out) - kept).abs() <= FRAME + 1e-3);
}

#[test]
fn the_thumbnail_strip_is_made_once_and_then_cached() {
    let dir = tempfile::tempdir().unwrap();
    let video = test_video(dir.path());
    let runs = Arc::new(Counting(AtomicUsize::new(0)));
    for _ in 0..2 {
        let mut r = open(dir.path(), &video, runs.clone());
        r.wait("the strip", PlayerWindow::has_strip);
    }
    assert_eq!(
        runs.0.load(Ordering::SeqCst),
        1,
        "the second window reused the cache"
    );
}

#[test]
fn pictures_of_the_paused_viewer() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let (_d, mut r) = rig();
        r.s.app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
        let (from, to) = (r.start_handle(), r.at(0.8).0 - f32::from(HANDLE_WIDTH) / 2.);
        r.drag(from, to);
        let (from, to) = (r.end_handle(), r.at(4.2).0 + f32::from(HANDLE_WIDTH) / 2.);
        r.drag(from, to);
        let (x, y) = r.at(2.6);
        r.s.click(x, y);
        r.wait("the strip", PlayerWindow::has_strip);
        std::thread::sleep(Duration::from_millis(200));
        r.s.app.settle();
        r.s.app.screenshot(&format!("player-paused-{name}"));
        let t = r.read(PlayerWindow::trim);
        assert!(
            (t.start() - 0.8).abs() < 0.05 && (t.end() - 4.2).abs() < 0.05,
            "{t:?}"
        );
    }
}

fn exported(video: &Path, name: &str, options: ExportOptions) -> PathBuf {
    let dir = video.parent().unwrap();
    let job = ExportJob {
        options,
        ..trimmed(video, dir.join(name), ExportMode::Precise, false)
    };
    export(&SystemFfmpeg, &job, &mut |_| {}).unwrap()
}

fn probed(file: &Path) -> es_record::recovery::VideoProbe {
    es_record::recovery::probe_video(&SystemRunner, file).unwrap()
}

#[test]
fn webm_and_gif_exports_have_their_codec_and_a_gif_has_no_audio() {
    let dir = tempfile::tempdir().unwrap();
    let video = test_video(dir.path());
    let webm = exported(
        &video,
        "clip.webm",
        ExportOptions {
            format: Format::WebM,
            quality: Quality::Small,
            ..Default::default()
        },
    );
    let p = probed(&webm);
    assert_eq!(p.codec, "vp9");
    assert_eq!((p.width, p.height), (640, 360));
    assert_eq!(audio_streams(&webm), "1");
    assert!((p.duration_secs - 2.5).abs() <= 2. * FRAME);

    let gif = exported(
        &video,
        "clip.gif",
        ExportOptions {
            format: Format::Gif,
            ..Default::default()
        },
    );
    let p = probed(&gif);
    assert_eq!(p.codec, "gif");
    assert_eq!(audio_streams(&gif), "", "a gif carries no audio");
}

#[test]
fn a_smaller_size_scales_down_and_a_larger_one_leaves_the_video_alone() {
    let dir = tempfile::tempdir().unwrap();
    let video = sized_video(dir.path(), "big.mkv", "1280x720");
    let small = exported(
        &video,
        "small.mp4",
        ExportOptions {
            size: Size::P480,
            ..Default::default()
        },
    );
    let p = probed(&small);
    assert_eq!((p.width, p.height), (854, 480));
    let same = exported(
        &video,
        "same.mp4",
        ExportOptions {
            size: Size::P1080,
            ..Default::default()
        },
    );
    let p = probed(&same);
    assert_eq!((p.width, p.height), (1280, 720));
    let lower = sized_video(dir.path(), "low.mkv", "640x360");
    let kept = exported(
        &lower,
        "kept.mp4",
        ExportOptions {
            size: Size::P720,
            ..Default::default()
        },
    );
    let p = probed(&kept);
    assert_eq!((p.width, p.height), (640, 360), "never scaled up");
}

#[test]
fn the_look_changes_the_exported_picture() {
    let dir = tempfile::tempdir().unwrap();
    let video = test_video(dir.path());
    let mono = exported(
        &video,
        "mono.mp4",
        ExportOptions {
            look: Look::Mono,
            ..Default::default()
        },
    );
    let args = [
        "-v",
        "error",
        "-ss",
        "1",
        "-i",
        mono.to_str().unwrap(),
        "-frames:v",
        "1",
        "-vf",
        "scale=1:1,format=rgb24",
        "-f",
        "rawvideo",
        "-",
    ]
    .map(String::from);
    let out = SystemRunner.run("ffmpeg", &args).unwrap();
    let px = out.stdout;
    assert_eq!(px.len(), 3);
    assert!(
        px[0].abs_diff(px[1]) <= 3 && px[1].abs_diff(px[2]) <= 3,
        "grey: {px:?}"
    );
}

#[test]
fn the_panel_previews_the_look_and_remembers_the_choices_at_export() {
    let (_d, mut r) = rig();
    let natural = r.read(|p| p.previewed_tone()).unwrap();
    assert_eq!(natural, (0., 1., 1.));
    let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let view = r.s.view.clone();
    let seen = events.clone();
    let _sub = r.s.app.cx.update(|cx| {
        cx.subscribe(&view, move |_, e: &PlayerEvent, _| {
            seen.borrow_mut().push(e.clone())
        })
    });
    let chosen = ExportOptions {
        format: Format::WebM,
        look: Look::Mono,
        brightness: 0.2,
        ..Default::default()
    };
    r.s.app
        .cx
        .update(|cx| view.update(cx, |p, cx| p.set_options(chosen, cx)));
    assert_eq!(r.read(|p| p.previewed_tone()).unwrap(), (0.2, 1.05, 0.));
    r.s.app
        .cx
        .update(|cx| view.update(cx, |p, cx| p.export(cx)));
    assert!(events.borrow().contains(&PlayerEvent::Chosen(chosen)));
    r.wait("the export", |p| {
        !matches!(p.export_state(), ExportState::Running(_))
    });
    assert_eq!(
        r.read(|p| p.export_state().clone()),
        ExportState::Done(r.dir.join("trimmed.webm"))
    );
}

#[test]
fn pictures_of_the_export_panel() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let (_d, mut r) = rig();
        r.s.app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
        let view = r.s.view.clone();
        let chosen = ExportOptions {
            format: Format::WebM,
            size: Size::P720,
            quality: Quality::High,
            look: Look::Vivid,
            brightness: 0.1,
        };
        r.s.app
            .cx
            .update(|cx| view.update(cx, |p, cx| p.set_options(chosen, cx)));
        r.wait("the strip", PlayerWindow::has_strip);
        std::thread::sleep(Duration::from_millis(200));
        r.s.app.settle();
        r.s.app.screenshot(&format!("player-export-panel-{name}"));
    }
}
