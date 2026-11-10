mod av;
mod av_support;
mod support;

use super::Encoder;
use crate::{Error, RecorderEvent, RecordingHandle};
use futures::executor::block_on;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use support::*;

const HD: (u32, u32) = (1280, 720);

fn frames(size: (u32, u32), secs: f64) -> Step {
    Step::Frames {
        size,
        rate: 60,
        span: Duration::from_secs_f64(secs),
    }
}

fn report(name: &str, rec: &super::WindowRecording) {
    eprintln!(
        "{name}: encoder {} stats {:?}",
        rec.encoder().element(),
        rec.stats()
    );
}

#[test]
fn ten_seconds_at_30_fps_are_one_frame_per_tick_and_last_as_long_as_the_recording() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![frames(HD, 30.0)]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    std::thread::sleep(Duration::from_secs(10));
    let stats = rec.stats();
    report("10 s", &rec);
    let out = block_on(Box::new(rec).stop()).expect("stop");
    let file = decode(&out.file, &[]);
    let recorded = spans_seconds(&out.segments);
    eprintln!(
        "10 s: file {} frames, {:.3} s; recorded {recorded:.3} s",
        file.frames,
        file.seconds()
    );
    assert_eq!(file.sizes, vec![HD]);
    assert!(
        file.frames.abs_diff(stats.ticks) <= 1,
        "{} frames for {} ticks",
        file.frames,
        stats.ticks
    );
    assert!(file.frames.abs_diff(300) <= 3, "{} frames", file.frames);
    let drift = (file.seconds() / recorded - 1.0).abs();
    assert!(
        drift < 0.02,
        "file {:.3} s, recorded {recorded:.3} s",
        file.seconds()
    );
    assert_eq!(stats.skipped_ticks, 0);
}

#[test]
fn a_window_that_never_changes_still_gives_a_full_length_constant_rate_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![Step::Frames {
        size: HD,
        rate: 1,
        span: Duration::from_millis(1),
    }]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    std::thread::sleep(Duration::from_secs(3));
    let stats = rec.stats();
    report("still", &rec);
    let out = block_on(Box::new(rec).stop()).expect("stop");
    let file = decode(&out.file, &[]);
    eprintln!(
        "still: file {} frames, {:.3} s",
        file.frames,
        file.seconds()
    );
    assert_eq!(stats.new_frames, 1);
    assert!(file.frames.abs_diff(stats.ticks) <= 1);
    assert!(file.frames.abs_diff(90) <= 3, "{} frames", file.frames);
    assert!(
        (file.seconds() - 3.0).abs() < 0.1,
        "{:.3} s",
        file.seconds()
    );
}

#[test]
fn a_resized_window_is_fitted_into_the_first_size_with_bars() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![frames(HD, 1.5), frames((360, 720), 30.0)]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    std::thread::sleep(Duration::from_secs(3));
    report("resize", &rec);
    let out = block_on(Box::new(rec).stop()).expect("stop");
    let file = decode(&out.file, &[SECOND / 2, SECOND * 5 / 2]);
    assert_eq!(file.sizes, vec![HD], "one size for the whole file");
    let [wide, tall] = &file.grabbed[..] else {
        panic!("two frames grabbed, got {}", file.grabbed.len());
    };
    let (edge, middle) = (column(tall, HD.0, 40), column(tall, HD.0, HD.0 / 2));
    eprintln!("resize: tall frame edge {edge:.1}, middle {middle:.1}");
    assert!(
        column(wide, HD.0, 40) > 80.0,
        "the first size fills the canvas"
    );
    assert!(edge < 16.0, "the narrow window is pillarboxed in black");
    assert!(middle > 80.0, "the narrow window is in the middle");
}

#[test]
fn a_pause_is_cut_out_of_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![frames(HD, 30.0)]);
    let mut rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    let events = rec.events();
    std::thread::sleep(Duration::from_secs(2));
    rec.pause().expect("pause");
    std::thread::sleep(Duration::from_secs(2));
    rec.resume().expect("resume");
    std::thread::sleep(Duration::from_secs(2));
    report("pause", &rec);
    let out = block_on(Box::new(rec).stop()).expect("stop");
    let file = decode(&out.file, &[]);
    let recorded = spans_seconds(&out.segments);
    eprintln!(
        "pause: file {} frames, {:.3} s; recorded {recorded:.3} s in {:?}",
        file.frames,
        file.seconds(),
        out.segments
    );
    assert_eq!(out.segments.len(), 2);
    assert!((recorded - 4.0).abs() < 0.1, "{recorded:.3} s recorded");
    let drift = (file.seconds() / recorded - 1.0).abs();
    assert!(drift < 0.02, "file {:.3} s", file.seconds());
    let seen: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert_eq!(
        seen,
        [
            RecorderEvent::Started,
            RecorderEvent::Paused,
            RecorderEvent::Resumed
        ]
    );
}

#[test]
fn a_closed_window_finishes_a_playable_file_and_says_the_window_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, dropped) = feed(vec![frames(HD, 1.5), Step::Stop]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    let events = rec.events();
    assert!(wait_for(&events, &RecorderEvent::WindowClosed));
    assert!(dropped.load(Ordering::SeqCst), "the capture is let go");
    let out = block_on(Box::new(rec).stop()).expect("the file is kept");
    let file = decode(&out.file, &[]);
    eprintln!(
        "closed: file {} frames, {:.3} s",
        file.frames,
        file.seconds()
    );
    assert!(file.frames > 20, "{} frames", file.frames);
    let drift = (file.seconds() / spans_seconds(&out.segments) - 1.0).abs();
    assert!(drift < 0.1, "file {:.3} s", file.seconds());
}

#[test]
fn a_capture_that_fails_while_recording_finishes_a_playable_file_and_says_the_window_was_lost() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, dropped) = feed(vec![frames(HD, 1.5), Step::Fail]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    let events = rec.events();
    assert!(wait_for(&events, &RecorderEvent::WindowLost));
    assert!(dropped.load(Ordering::SeqCst), "the capture is let go");
    let out = block_on(Box::new(rec).stop()).expect("the file is kept");
    let file = decode(&out.file, &[]);
    assert!(file.frames > 20, "{} frames", file.frames);
}

#[test]
fn discard_leaves_no_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("rec");
    let (feed, dropped) = feed(vec![frames(HD, 30.0)]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(&folder, 30))
        .expect("start");
    assert!(folder.join("screen.mkv").exists());
    std::thread::sleep(Duration::from_millis(500));
    block_on(Box::new(rec).discard()).expect("discard");
    assert!(!folder.exists());
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn dropping_the_recording_ends_its_capture_and_finishes_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, dropped) = feed(vec![frames(HD, 30.0)]);
    let rec = recorder(feed, &test_encoders())
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    std::thread::sleep(Duration::from_secs(1));
    drop(rec);
    let until = Instant::now() + Duration::from_secs(5);
    while !dropped.load(Ordering::SeqCst) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(dropped.load(Ordering::SeqCst));
    std::thread::sleep(Duration::from_millis(500));
    let file = decode(&dir.path().join("screen.mkv"), &[]);
    assert!(file.frames >= 25, "{} frames", file.frames);
}

#[test]
fn a_window_closed_before_its_first_frame_does_not_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![Step::Stop]);
    let started = recorder(feed, &test_encoders()).start_window(&spec(dir.path(), 30));
    assert!(matches!(started, Err(Error::WindowClosed)));
}

#[test]
fn the_default_order_records_with_the_first_encoder_that_works() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![frames(HD, 30.0)]);
    let rec = recorder(feed, &Encoder::ORDER)
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    std::thread::sleep(Duration::from_secs(2));
    let stats = rec.stats();
    report("default order", &rec);
    let out = block_on(Box::new(rec).stop()).expect("stop");
    let file = decode(&out.file, &[]);
    assert_eq!(file.sizes, vec![HD]);
    assert!(file.frames.abs_diff(stats.ticks) <= 1);
}

#[test]
fn an_encoder_that_fails_on_the_first_frame_falls_through_to_the_next() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Smaller than the 160x64 nvh264enc accepts, so it fails whether or not a GPU is present.
    let (feed, _) = feed(vec![frames((100, 50), 30.0)]);
    let rec = recorder(feed, &[Encoder::Nvh264, Encoder::X264])
        .start_window(&spec(dir.path(), 30))
        .expect("start");
    assert_eq!(rec.encoder(), Encoder::X264);
    std::thread::sleep(Duration::from_millis(500));
    let out = block_on(Box::new(rec).stop()).expect("stop");
    assert_eq!(decode(&out.file, &[]).sizes, vec![(100, 50)]);
}
