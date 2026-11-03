#![allow(clippy::unwrap_used)]

use crate::input_keys::us_text;
use es_geom::{Point, Rect};
use es_model::platform::{CursorSource, KeymapSource, KeymapText};
use es_model::recording::{InputAccess, PressPhase};
use es_record::input::{
    EventSource, HeldKeys, InputBuilder, InputCollector, KeyFilter, Keymap, RawEvent, Sources,
    access, is_event_node,
};
use es_record::{Clock, RecordingOutput, SystemClock, movie_time};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIRST: u64 = 1_000_000;
const ESC: u16 = 1;
const ENTER: u16 = 28;
const CTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_C: u16 = 46;

fn out(segments: &[(u64, u64)]) -> RecordingOutput {
    RecordingOutput {
        file: PathBuf::from("screen.mkv"),
        first_frame_monotonic_us: FIRST,
        segments: segments.to_vec(),
    }
}

fn rect() -> Rect {
    Rect::new(100.0, 200.0, 300.0, 400.0)
}

fn key(code: u16, down: bool, t_us: u64) -> RawEvent {
    RawEvent::Key {
        device: 0,
        code,
        down,
        t_us,
    }
}

fn button(down: bool, t_us: u64) -> RawEvent {
    RawEvent::Button {
        code: 0x110,
        down,
        t_us,
    }
}

fn us() -> KeyFilter {
    KeyFilter::new(Keymap::from_text(&us_text()))
}

#[test]
fn a_key_whose_release_was_lost_stops_counting_once_the_device_says_it_is_up() {
    let mut held = HeldKeys::default();
    let mut keys = us();
    let mut b = InputBuilder::new(rect());
    let mut feed = |b: &mut InputBuilder, keys: &mut KeyFilter, ev: RawEvent| {
        if let RawEvent::Key { code, down, .. } = ev {
            held.note(code, down);
        }
        b.event(ev, None, keys);
    };
    feed(&mut b, &mut keys, key(CTRL, true, FIRST + 1));
    feed(&mut b, &mut keys, key(KEY_C, true, FIRST + 2));
    for ev in held.lost(0, |_| false, FIRST + 3) {
        b.event(ev, None, &mut keys);
    }
    assert!(held.is_empty());
    b.event(key(KEY_A, true, FIRST + 4), None, &mut keys);
    b.event(key(KEY_A, false, FIRST + 5), None, &mut keys);
    let stored = b.finish(&out(&[(FIRST, FIRST + 1_000_000)])).keystrokes;
    assert_eq!(stored.len(), 1, "only the real Ctrl+C: {stored:?}");
    assert_eq!(stored[0].key, "C");

    let mut still = HeldKeys::default();
    still.note(CTRL, true);
    still.note(KEY_A, true);
    assert_eq!(still.lost(0, |c| c == CTRL, 0).len(), 1, "A is up");
    assert!(!still.is_empty());
}

#[test]
fn presses_inside_the_area_are_normalised_and_presses_outside_it_are_dropped() {
    let mut keys = KeyFilter::default();
    let mut b = InputBuilder::new(rect());
    let at = |x, y| Some(Point::new(x, y));
    b.event(button(true, FIRST + 200_000), at(200.0, 300.0), &mut keys);
    b.event(button(false, FIRST + 400_000), at(300.0, 400.0), &mut keys);
    b.event(button(true, FIRST + 450_000), at(150.0, 900.0), &mut keys);
    b.event(button(true, FIRST + 460_000), at(50.0, 300.0), &mut keys);
    b.event(button(true, FIRST + 500_000), None, &mut keys);
    let touch = RawEvent::Button {
        code: 0x14a,
        down: true,
        t_us: FIRST + 600_000,
    };
    b.event(touch, None, &mut keys);
    let i = b.finish(&out(&[(FIRST, FIRST + 9_000_000)]));
    assert!(i.travel.is_empty());
    let p = &i.presses;
    assert_eq!(p.len(), 3, "outside the area or a touch contact: {p:?}");
    assert_eq!((p[0].x, p[0].y, p[0].button), (Some(0.5), Some(0.5), 0));
    assert_eq!(p[0].phase, PressPhase::Down);
    assert_eq!((p[1].x, p[1].y), (Some(1.0), Some(1.0)), "the far edge");
    assert_eq!(p[1].phase, PressPhase::Up);
    assert_eq!((p[2].x, p[2].y), (None, None));
}

#[test]
fn paused_time_is_subtracted_and_key_times_are_coarse() {
    let segs = [
        (FIRST, FIRST + 1_000_000),
        (FIRST + 3_000_000, FIRST + 4_000_000),
    ];
    let o = out(&segs);
    assert_eq!(movie_time(FIRST + 500_000, &o), Some(0.5));
    assert_eq!(movie_time(FIRST + 2_000_000, &o), None);
    assert_eq!(movie_time(FIRST + 3_500_000, &o), Some(1.5));
    assert_eq!(movie_time(FIRST - 1, &o), None);

    let mut keys = KeyFilter::default();
    let mut b = InputBuilder::new(rect());
    b.event(button(true, FIRST + 100_000), None, &mut keys);
    b.event(key(ENTER, true, FIRST + 2_000_000), None, &mut keys);
    b.event(key(ESC, true, FIRST + 3_503_721), None, &mut keys);
    let i = b.finish(&o);
    assert_eq!(i.presses.iter().map(|p| p.time).collect::<Vec<_>>(), [0.1]);
    let stored: Vec<_> = i.keystrokes.iter().map(|k| (k.time, &*k.key)).collect();
    assert_eq!(stored, [(1.5, "Esc")]);
}

#[test]
fn input_json_has_the_documented_shape() {
    let mut keys = us();
    let mut b = InputBuilder::new(rect());
    b.event(
        button(true, FIRST + 200_000),
        Some(Point::new(200.0, 300.0)),
        &mut keys,
    );
    b.event(button(false, FIRST + 300_000), None, &mut keys);
    b.event(key(CTRL, true, FIRST + 400_000), None, &mut keys);
    b.event(key(KEY_C, true, FIRST + 500_000), None, &mut keys);
    let i = b.finish(&out(&[(FIRST, FIRST + 1_000_000)]));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.json");
    es_record::write_input(&path, &i).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(v["travel"], serde_json::json!([]));
    assert_eq!(v["presses"][0]["phase"], "down");
    assert_eq!(v["presses"][0]["x"], 0.5);
    assert!(v["presses"][1].get("x").is_none());
    assert_eq!(v["keystrokes"][0]["key"], "C");
    assert_eq!(v["keystrokes"][0]["modifiers"][0], "⌃");
    assert_eq!(v["is_sanitized"], false);
}

struct Still(Arc<Mutex<usize>>);
impl CursorSource for Still {
    fn sample(&self) -> Option<(Point, Instant)> {
        *self.0.lock().unwrap() += 1;
        Some((Point::new(200.0, 300.0), Instant::now()))
    }
}

/// Samples only through a stream it opens, and counts the streams still alive.
struct Streaming {
    opened: Mutex<Vec<std::sync::Weak<dyn CursorSource>>>,
    samples: Arc<Mutex<usize>>,
}
impl CursorSource for Streaming {
    fn sample(&self) -> Option<(Point, Instant)> {
        None
    }
    fn open(&self) -> Option<Arc<dyn CursorSource>> {
        let s: Arc<dyn CursorSource> = Arc::new(Still(self.samples.clone()));
        self.opened.lock().unwrap().push(Arc::downgrade(&s));
        Some(s)
    }
}
impl Streaming {
    fn alive(&self) -> usize {
        let opened = self.opened.lock().unwrap();
        opened.iter().filter(|w| w.strong_count() > 0).count()
    }
}

struct Fixed(Mutex<Option<KeymapText>>);
impl KeymapSource for Fixed {
    fn keymap(&self) -> Option<KeymapText> {
        self.0.lock().unwrap().clone()
    }
}

struct Scripted(Arc<Mutex<Vec<RawEvent>>>);
impl EventSource for Scripted {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        std::thread::sleep(wait);
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

struct Rig {
    clock: SystemClock,
    queue: Arc<Mutex<Vec<RawEvent>>>,
    open: Arc<AtomicBool>,
    keymap: Arc<Fixed>,
    first: u64,
}

impl Rig {
    fn start(cursor: Arc<dyn CursorSource>) -> (Rig, InputCollector) {
        let clock = SystemClock::default();
        let rig = Rig {
            first: clock.monotonic_us(),
            clock,
            queue: Arc::new(Mutex::new(Vec::new())),
            open: Arc::new(AtomicBool::new(true)),
            keymap: Arc::new(Fixed(Mutex::new(Some(KeymapText {
                serial: 1,
                text: us_text().into(),
            })))),
        };
        let sources = Sources {
            cursor: Some(cursor),
            keymap: Some(rig.keymap.clone()),
            session_open: rig.open.clone(),
        };
        let c = InputCollector::start(rect(), sources, Box::new(Scripted(rig.queue.clone())));
        (rig, c)
    }

    fn ctrl_c(&self) {
        let now = self.clock.monotonic_us();
        self.queue.lock().unwrap().extend([
            key(CTRL, true, now),
            key(KEY_C, true, now),
            key(KEY_C, false, now),
            key(CTRL, false, now),
        ]);
        std::thread::sleep(Duration::from_millis(80));
    }

    fn finish(&self, c: InputCollector) -> es_model::recording::RecordingInput {
        let end = self.clock.monotonic_us();
        c.stop().unwrap().finish(&RecordingOutput {
            file: PathBuf::new(),
            first_frame_monotonic_us: self.first,
            segments: vec![(self.first, end)],
        })
    }
}

#[test]
fn the_collector_reads_a_fake_source_and_asks_for_the_pointer_only_on_clicks() {
    let samples = Arc::new(Mutex::new(0));
    let (rig, c) = Rig::start(Arc::new(Still(samples.clone())));
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(*samples.lock().unwrap(), 0, "no pointer polling while idle");
    let now = rig.clock.monotonic_us();
    rig.queue.lock().unwrap().extend([
        button(true, now),
        key(CTRL, true, now),
        key(KEY_C, true, now),
    ]);
    std::thread::sleep(Duration::from_millis(80));
    let i = rig.finish(c);
    assert_eq!(i.presses.len(), 1);
    assert_eq!(i.presses[0].x, Some(0.5));
    assert_eq!(i.keystrokes.len(), 1);
}

#[test]
fn nothing_is_read_while_the_session_is_inactive_or_locked() {
    let (rig, c) = Rig::start(Arc::new(Still(Arc::default())));
    rig.ctrl_c();
    rig.open.store(false, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(40));
    let now = rig.clock.monotonic_us();
    rig.queue
        .lock()
        .unwrap()
        .extend([key(CTRL, true, now), button(true, now)]);
    rig.ctrl_c();
    rig.open.store(true, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(40));
    let now = rig.clock.monotonic_us();
    rig.queue.lock().unwrap().push(key(KEY_A, true, now));
    rig.ctrl_c();
    let i = rig.finish(c);
    assert!(i.presses.is_empty(), "{:?}", i.presses);
    let stored: Vec<_> = i.keystrokes.iter().map(|k| &*k.key).collect();
    assert_eq!(stored, ["C", "C"], "a Ctrl held while locked is forgotten");
}

#[test]
fn the_collector_follows_the_keymap_and_stores_no_letters_without_one() {
    let (rig, c) = Rig::start(Arc::new(Still(Arc::default())));
    *rig.keymap.0.lock().unwrap() = None;
    rig.ctrl_c();
    *rig.keymap.0.lock().unwrap() = Some(KeymapText {
        serial: 2,
        text: us_text().into(),
    });
    rig.ctrl_c();
    let i = rig.finish(c);
    assert_eq!(i.keystrokes.len(), 1, "{:?}", i.keystrokes);
}

#[test]
fn halting_stops_reading_at_once() {
    let (rig, c) = Rig::start(Arc::new(Still(Arc::default())));
    c.halt();
    std::thread::sleep(Duration::from_millis(40));
    rig.ctrl_c();
    assert!(rig.finish(c).keystrokes.is_empty());
}

#[test]
fn a_streaming_cursor_is_open_only_while_the_collector_runs() {
    let cursor = Arc::new(Streaming {
        opened: Mutex::default(),
        samples: Arc::default(),
    });
    assert_eq!(cursor.alive(), 0);
    let (rig, c) = Rig::start(cursor.clone());
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(cursor.alive(), 1);
    let now = rig.clock.monotonic_us();
    rig.queue.lock().unwrap().push(button(true, now));
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(
        *cursor.samples.lock().unwrap(),
        1,
        "sampled through the stream"
    );
    let i = rig.finish(c);
    assert_eq!(i.presses[0].x, Some(0.5));
    assert_eq!(cursor.alive(), 0, "closed with the recording");
}

#[test]
fn only_event_nodes_are_candidate_devices() {
    assert!(is_event_node("event0") && is_event_node("event17"));
    assert!(!is_event_node("mouse0") && !is_event_node("event") && !is_event_node("event1x"));
}

#[cfg(feature = "evdev-input")]
#[test]
fn access_is_granted_only_when_every_event_node_is_readable() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(access(dir.path()), InputAccess::Denied, "no devices");
    let set = |name: &str, mode: u32| {
        let p = dir.path().join(name);
        std::fs::write(&p, b"").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    set("event0", 0o644);
    set("mouse0", 0o000);
    assert_eq!(access(dir.path()), InputAccess::Granted);
    set("event1", 0o000);
    assert_eq!(access(dir.path()), InputAccess::Denied);
    assert_eq!(
        access(&dir.path().join("missing")),
        InputAccess::Denied,
        "an unreadable directory"
    );
}

#[cfg(not(feature = "evdev-input"))]
#[test]
fn without_the_feature_access_says_the_build_lacks_it() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(access(dir.path()), InputAccess::NotBuilt);
}
