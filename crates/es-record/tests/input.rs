#![allow(clippy::unwrap_used)]

use es_geom::{Point, Rect};
use es_model::platform::CursorSource;
use es_model::recording::{PressPhase, TravelKind};
use es_record::input::{EventSource, InputBuilder, InputCollector, InputConfig, RawEvent};
use es_record::{Clock, RecordingOutput, SystemClock, movie_time};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIRST: u64 = 1_000_000;
const CTRL: u16 = 29;
const SHIFT: u16 = 42;
const KEY_C: u16 = 46;
const KEY_A: u16 = 30;
const ENTER: u16 = 28;

fn out(segments: &[(u64, u64)]) -> RecordingOutput {
    RecordingOutput {
        file: PathBuf::from("screen.mkv"),
        first_frame_monotonic_us: FIRST,
        segments: segments.to_vec(),
    }
}

fn cfg(keystrokes: bool) -> InputConfig {
    InputConfig {
        capture_rect: Rect::new(100.0, 200.0, 300.0, 400.0),
        keystrokes,
    }
}

fn key(code: u16, down: bool, t_us: u64) -> RawEvent {
    RawEvent::Key { code, down, t_us }
}

fn button(down: bool, t_us: u64) -> RawEvent {
    RawEvent::Button {
        code: 0x110,
        down,
        t_us,
    }
}

fn typed(b: &mut InputBuilder, seq: &[(u16, bool)]) {
    for (n, &(code, down)) in seq.iter().enumerate() {
        b.event(key(code, down, FIRST + 1000 * (n as u64 + 1)));
    }
}

#[test]
fn positions_are_normalised_to_the_capture_rect_and_clamped() {
    let mut b = InputBuilder::new(cfg(false));
    b.cursor(Point::new(200.0, 300.0), FIRST + 500_000);
    b.cursor(Point::new(0.0, 900.0), FIRST + 600_000);
    let i = b.finish(&out(&[(FIRST, FIRST + 5_000_000)]));
    assert_eq!(i.travel.len(), 2);
    assert_eq!(
        (i.travel[0].x, i.travel[0].y, i.travel[0].time),
        (0.5, 0.5, 0.5)
    );
    assert_eq!((i.travel[1].x, i.travel[1].y), (0.0, 1.0));
}

#[test]
fn paused_time_is_subtracted_and_events_inside_a_pause_are_dropped() {
    let segs = [
        (FIRST, FIRST + 1_000_000),
        (FIRST + 3_000_000, FIRST + 4_000_000),
    ];
    let o = out(&segs);
    assert_eq!(movie_time(FIRST + 500_000, &o), Some(0.5));
    assert_eq!(movie_time(FIRST + 2_000_000, &o), None);
    assert_eq!(movie_time(FIRST + 3_500_000, &o), Some(1.5));
    assert_eq!(movie_time(FIRST - 1, &o), None);

    let mut b = InputBuilder::new(cfg(false));
    b.cursor(Point::new(150.0, 250.0), FIRST + 100_000);
    b.cursor(Point::new(160.0, 250.0), FIRST + 2_000_000);
    b.cursor(Point::new(170.0, 250.0), FIRST + 3_500_000);
    let i = b.finish(&o);
    let times: Vec<f64> = i.travel.iter().map(|t| t.time).collect();
    assert_eq!(times, [0.1, 1.5]);
}

#[test]
fn presses_carry_the_pointer_position_and_drags_are_marked() {
    let mut b = InputBuilder::new(cfg(false));
    b.cursor(Point::new(200.0, 300.0), FIRST + 100_000);
    b.event(button(true, FIRST + 200_000));
    b.cursor(Point::new(250.0, 300.0), FIRST + 300_000);
    b.event(button(false, FIRST + 400_000));
    b.cursor(Point::new(260.0, 300.0), FIRST + 500_000);
    let i = b.finish(&out(&[(FIRST, FIRST + 9_000_000)]));
    assert_eq!(i.presses.len(), 2);
    assert_eq!(i.presses[0].phase, PressPhase::Down);
    assert_eq!(
        (i.presses[0].x, i.presses[0].y, i.presses[0].button),
        (0.5, 0.5, 0)
    );
    assert_eq!(i.presses[1].phase, PressPhase::Up);
    let kinds: Vec<_> = i.travel.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [TravelKind::Move, TravelKind::Drag, TravelKind::Move]
    );
}

#[test]
fn plain_typing_is_never_stored_but_chords_and_special_keys_are() {
    let mut b = InputBuilder::new(cfg(true));
    typed(
        &mut b,
        &[
            (KEY_A, true),
            (KEY_A, false),
            (SHIFT, true),
            (KEY_A, true),
            (KEY_A, false),
            (SHIFT, false),
            (CTRL, true),
            (KEY_C, true),
            (KEY_C, false),
            (CTRL, false),
            (ENTER, true),
            (ENTER, false),
        ],
    );
    let i = b.finish(&out(&[(FIRST, FIRST + 1_000_000)]));
    let got: Vec<(Vec<String>, String)> = i
        .keystrokes
        .iter()
        .map(|k| (k.modifiers.clone(), k.key.clone()))
        .collect();
    assert_eq!(
        got,
        [
            (vec!["⌃".to_owned()], "C".to_owned()),
            (Vec::new(), "Enter".to_owned())
        ]
    );
}

#[test]
fn keystrokes_are_off_unless_enabled() {
    let mut b = InputBuilder::new(cfg(false));
    typed(&mut b, &[(CTRL, true), (KEY_C, true), (ENTER, true)]);
    assert!(
        b.finish(&out(&[(FIRST, FIRST + 1_000_000)]))
            .keystrokes
            .is_empty()
    );
}

#[test]
fn input_json_has_the_documented_shape() {
    let mut b = InputBuilder::new(cfg(true));
    b.cursor(Point::new(200.0, 300.0), FIRST + 100_000);
    b.event(button(true, FIRST + 200_000));
    typed(&mut b, &[(CTRL, true), (KEY_C, true)]);
    let i = b.finish(&out(&[(FIRST, FIRST + 1_000_000)]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.json");
    es_record::write_input(&path, &i).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(v["travel"][0]["kind"], "move");
    assert_eq!(v["presses"][0]["phase"], "down");
    assert_eq!(v["keystrokes"][0]["key"], "C");
    assert_eq!(v["keystrokes"][0]["modifiers"][0], "⌃");
    assert_eq!(v["is_sanitized"], false);
}

struct Still;
impl CursorSource for Still {
    fn sample(&self) -> Option<(Point, Instant)> {
        Some((Point::new(200.0, 300.0), Instant::now()))
    }
}

struct Scripted(Arc<Mutex<Vec<RawEvent>>>);
impl EventSource for Scripted {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        std::thread::sleep(wait);
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[test]
fn the_collector_samples_the_cursor_and_reads_a_fake_event_source() {
    let clock = Arc::new(SystemClock::default());
    let queue = Arc::new(Mutex::new(Vec::new()));
    let c = InputCollector::start(
        cfg(true),
        Arc::new(Still),
        clock.clone(),
        Some(Box::new(Scripted(queue.clone()))),
    );
    let first = clock.monotonic_us();
    std::thread::sleep(Duration::from_millis(80));
    queue.lock().unwrap().extend([
        button(true, clock.monotonic_us()),
        key(CTRL, true, clock.monotonic_us()),
        key(KEY_C, true, clock.monotonic_us()),
    ]);
    std::thread::sleep(Duration::from_millis(80));
    let end = clock.monotonic_us();
    let i = c
        .finish(&RecordingOutput {
            file: PathBuf::new(),
            first_frame_monotonic_us: first,
            segments: vec![(first, end)],
        })
        .unwrap();
    assert!(!i.travel.is_empty());
    assert_eq!(i.presses.len(), 1);
    assert_eq!(i.keystrokes.len(), 1);
}

#[cfg(feature = "evdev-input")]
#[test]
fn only_event_nodes_are_candidate_devices() {
    use es_record::input::is_event_node;
    assert!(is_event_node("event0") && is_event_node("event17"));
    assert!(!is_event_node("mouse0") && !is_event_node("event") && !is_event_node("event1x"));
}
