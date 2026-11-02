#![cfg(feature = "portal-recorder")]
#![allow(clippy::unwrap_used)]

use es_record::portal::{
    CaptureSource, ENCODERS, GstCapture, element_available, gst_init, pick_encoder,
};
use es_record::recovery::probe_video;
use es_record::{RecordingHandle, SystemClock, SystemRunner};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn encoder_fallback_follows_the_documented_order() {
    let order: Vec<_> = ENCODERS.iter().map(|e| e.element).collect();
    assert_eq!(
        order,
        ["vah264enc", "nvh264enc", "x264enc", "openh264enc", "vp9enc"]
    );
    let only = |names: &'static [&'static str]| pick_encoder(move |n| names.contains(&n));
    assert_eq!(
        only(&["vp9enc", "x264enc", "vah264enc"]).unwrap().element,
        "vah264enc"
    );
    assert_eq!(
        only(&["vp9enc", "openh264enc", "x264enc"]).unwrap().element,
        "x264enc"
    );
    assert_eq!(only(&["vp9enc"]).unwrap().element, "vp9enc");
    assert!(only(&["theoraenc"]).is_none());
}

fn pipeline_ready() -> bool {
    gst_init().is_ok()
        && ["videotestsrc", "videoconvert", "matroskamux", "filesink"]
            .iter()
            .all(|e| element_available(e))
        && pick_encoder(element_available).is_some()
        && std::process::Command::new("ffprobe")
            .arg("-version")
            .output()
            .is_ok()
}

#[test]
fn a_test_source_pipeline_writes_a_playable_mkv_with_the_pause_removed() {
    if !pipeline_ready() {
        eprintln!("skipped: GStreamer plugins or ffprobe missing");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("screen.mkv");
    let mut cap = GstCapture::start(
        CaptureSource::Test {
            width: 320,
            height: 240,
            fps: 15,
        },
        &out,
        None,
        Arc::new(SystemClock::default()),
        None,
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(600));
    cap.pause().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    cap.resume().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let output = futures::executor::block_on(Box::new(cap).stop()).unwrap();

    assert_eq!(output.segments.len(), 2);
    assert!(output.first_frame_monotonic_us > 0);
    let probe = probe_video(&SystemRunner, &out).expect("ffprobe reads the file");
    assert_eq!((probe.width, probe.height), (320, 240));
    assert!(
        probe.duration_secs > 0.7,
        "duration {}",
        probe.duration_secs
    );
    assert!(
        probe.duration_secs < 1.4,
        "the 0.5 s pause must not be in the movie: {}",
        probe.duration_secs
    );
}
