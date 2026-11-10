use super::super::audio::pulse_source;
use super::super::pipeline::bitrate_kbps;
use super::super::{Encoder, WindowRecorder};
use super::av_support::*;
use super::support::*;
use crate::mute::AudioSource;
use crate::{Error, Quality, RecordSpec, RecorderEvent, RecordingHandle, RecordingOutput};
use futures::executor::block_on;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

const SIZE: (u32, u32) = (640, 360);
const MS: f64 = 0.001;

fn video(span: f64) -> Step {
    Step::Frames {
        size: SIZE,
        rate: 60,
        span: Duration::from_secs_f64(span),
    }
}

fn sound_spec(dir: &Path, system: bool, mic: bool) -> RecordSpec {
    let mut s = spec(dir, 30);
    s.system_audio = system;
    s.mic = mic.then(|| "usb-mic".to_owned());
    s
}

fn tones(source: AudioSource) -> gst::Element {
    match source {
        AudioSource::System => tone(1000.0),
        AudioSource::Mic => tone(440.0),
    }
}

fn with_tones(feed: Feed) -> WindowRecorder {
    recorder(feed, &test_encoders()).with_audio_sources(sources(tones))
}

fn stop(rec: Box<dyn RecordingHandle>) -> RecordingOutput {
    block_on(rec.stop()).expect("stop")
}

fn sleep(secs: f64) {
    std::thread::sleep(Duration::from_secs_f64(secs));
}

/// Each track ends within a frame and an encoded sound frame of the video, and has no holes.
fn assert_runs_with_the_video(av: &Av) {
    for (i, s) in av.sound.iter().enumerate() {
        eprintln!(
            "track {i}: {:.3} s from {:.3} s, widest gap {} us; video {:.3} s",
            s.seconds(),
            s.start as f64 / 1e9,
            s.widest_gap / 1000,
            av.video_seconds()
        );
        assert!(s.start < 25_000_000, "track {i} starts at the first frame");
        assert!(s.widest_gap < 2_000_000, "track {i} is continuous");
        let off = s.seconds() - av.video_seconds();
        assert!(
            off.abs() < 60.0 * MS,
            "track {i} ends {off:.3} s off the video"
        );
    }
}

#[test]
fn system_sound_and_the_microphone_are_two_tracks_in_that_order_as_long_as_the_video() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![video(30.0)]);
    let rec = with_tones(feed)
        .start_window(&sound_spec(dir.path(), true, true))
        .expect("start");
    sleep(3.0);
    let out = stop(Box::new(rec));
    let av = decode_av(&out.file);
    assert_eq!(av.sound.len(), 2, "one track per source");
    assert_runs_with_the_video(&av);
    let (system, mic) = (av.sound[0].pitch(0.5, 2.5), av.sound[1].pitch(0.5, 2.5));
    eprintln!("tracks: first {system:.0} Hz, second {mic:.0} Hz");
    assert!((system - 1000.0).abs() < 20.0, "system sound is first");
    assert!((mic - 440.0).abs() < 10.0, "the microphone is second");
    assert!((av.video_seconds() - 3.0).abs() < 0.1);
}

#[test]
fn a_flash_and_a_click_at_one_moment_stay_within_a_frame_after_a_pause() {
    let dir = tempfile::tempdir().expect("tempdir");
    let at = Arc::new(AtomicU64::new(0));
    let (feed, _) = feed(vec![Step::Flash {
        size: SIZE,
        at: at.clone(),
        span: Duration::from_secs(30),
    }]);
    let clicks = at.clone();
    let mut rec = recorder(feed, &test_encoders())
        .with_audio_sources(sources(move |_| click(clicks.clone())))
        .start_window(&sound_spec(dir.path(), true, false))
        .expect("start");
    sleep(1.0);
    rec.pause().expect("pause");
    sleep(2.0);
    rec.resume().expect("resume");
    sleep(0.5);
    // Half a frame off the ticks, which the sleeps above keep in step with.
    at.store(clock_now() + 1_016_000_000, Ordering::SeqCst);
    sleep(2.0);
    let out = stop(Box::new(rec));
    let av = decode_av(&out.file);
    assert_runs_with_the_video(&av);
    let flash = av
        .video
        .iter()
        .find(|(_, luma)| *luma > 128.0)
        .map(|(pts, _)| *pts as f64 / 1e9)
        .expect("the flash is in the file");
    let click = av.sound[0].first_loud().expect("the click is in the file");
    let frame = 1.0 / 30.0;
    eprintln!(
        "flash at {flash:.4} s, click at {click:.4} s, flash - click {:.1} ms",
        (flash - click) / MS
    );
    assert!(
        (click - 2.516).abs() < 0.01,
        "1 s before the pause and 1.516 s after it"
    );
    assert!(
        (-4.0 * MS..=frame + 4.0 * MS).contains(&(flash - click)),
        "the flash shows in the frame that covers the click"
    );
}

#[test]
fn a_muted_source_records_silence_and_its_track_runs_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![video(30.0)]);
    let mut rec = with_tones(feed)
        .start_window(&sound_spec(dir.path(), true, false))
        .expect("start");
    assert!(matches!(
        rec.set_muted(AudioSource::Mic, true),
        Err(Error::NotFound(_))
    ));
    sleep(1.5);
    rec.set_muted(AudioSource::System, true).expect("mute");
    sleep(1.5);
    rec.set_muted(AudioSource::System, false).expect("unmute");
    sleep(1.5);
    let av = decode_av(&stop(Box::new(rec)).file);
    assert_runs_with_the_video(&av);
    let s = &av.sound[0];
    let (before, muted, after) = (s.rms(0.2, 1.3), s.rms(1.8, 2.8), s.rms(3.3, 4.3));
    eprintln!("mute: rms {before:.0}, then {muted:.0}, then {after:.0}");
    assert!(before > 5_000.0 && after > 5_000.0, "sound around the mute");
    assert!(muted < 50.0, "silence while muted");
}

#[test]
fn an_unplugged_microphone_says_so_and_its_track_goes_on_silent() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (feed, _) = feed(vec![video(30.0)]);
    let server = Arc::new(Unplugged {
        mic: "usb-mic".into(),
        polls: AtomicUsize::new(2),
    });
    let rec = with_tones(feed)
        .with_audio_runner(server)
        .start_window(&sound_spec(dir.path(), false, true))
        .expect("start");
    let events = rec.events();
    assert!(wait_for(&events, &RecorderEvent::MicLost));
    sleep(1.5);
    let seen: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert!(
        !seen.iter().any(|e| matches!(e, RecorderEvent::Failed(_))),
        "the recording goes on: {seen:?}"
    );
    let out = stop(Box::new(rec));
    let av = decode_av(&out.file);
    assert_runs_with_the_video(&av);
    let s = &av.sound[0];
    let end = s.seconds();
    let (plugged, unplugged) = (s.rms(0.3, 1.5), s.rms(end - 1.0, end - 0.1));
    eprintln!("mic lost: rms {plugged:.0}, then {unplugged:.0}");
    assert!(
        plugged > 5_000.0,
        "the microphone is heard while it is there"
    );
    assert!(unplugged < 50.0, "silence once it is gone");
}

#[test]
fn bitrate_rises_with_quality_size_and_rate_and_stays_in_bounds() {
    let hd = (1920, 1080);
    let by_quality: Vec<u32> = [
        Quality::Medium,
        Quality::High,
        Quality::VeryHigh,
        Quality::Ultra,
    ]
    .into_iter()
    .map(|q| bitrate_kbps(q, hd, 30))
    .collect();
    eprintln!("1080p30 kbit/s by quality: {by_quality:?}");
    assert!(by_quality.windows(2).all(|w| w[0] < w[1]));
    assert!((3_000..=8_000).contains(&by_quality[1]), "1080p30 high");
    assert!(bitrate_kbps(Quality::High, hd, 60) > by_quality[1]);
    assert!(bitrate_kbps(Quality::High, (3840, 2160), 30) > by_quality[1]);
    assert_eq!(bitrate_kbps(Quality::Medium, (200, 100), 30), 1_000);
    assert_eq!(bitrate_kbps(Quality::Ultra, (7680, 4320), 60), 80_000);
}

#[test]
fn each_encoder_takes_the_bitrate_it_is_given() {
    gst::init().expect("gstreamer");
    for encoder in Encoder::ORDER.into_iter().filter(|e| e.installed()) {
        let bin = gst::parse::bin_from_description(
            &format!("videoconvert ! {}", encoder.chain(4_321)),
            true,
        )
        .unwrap_or_else(|e| panic!("{}: {e}", encoder.element()));
        let element = bin.by_name("encoder").expect("encoder");
        let kbps: u32 = element.property("bitrate");
        eprintln!("{}: {kbps} kbit/s", encoder.element());
        assert_eq!(kbps, 4_321, "{}", encoder.element());
    }
}

#[test]
fn each_source_is_its_own_named_sound_server_stream() {
    gst::init().expect("gstreamer");
    if gst::ElementFactory::find("pulsesrc").is_none() {
        eprintln!("pulsesrc not installed");
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let mut s = sound_spec(dir.path(), true, true);
    s.mic = Some("device:alsa_input.usb".into());
    // Built only, never started: nothing connects to a sound server.
    let system = pulse_source(AudioSource::System, &s).expect("system");
    let mic = pulse_source(AudioSource::Mic, &s).expect("mic");
    let device = |e: &gst::Element| e.property::<Option<String>>("device");
    let client = |e: &gst::Element| e.property::<Option<String>>("client-name");
    assert_eq!(device(&system).as_deref(), Some("@DEFAULT_MONITOR@"));
    assert_eq!(device(&mic).as_deref(), Some("alsa_input.usb"));
    assert_ne!(client(&system), client(&mic));
    s.mic = Some("default".into());
    let default = pulse_source(AudioSource::Mic, &s).expect("default mic");
    assert_eq!(device(&default), None, "the server's default source");
}
