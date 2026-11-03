#![allow(clippy::unwrap_used)]

use es_geom::{Point, Rect};
use es_model::platform::CursorSource;
use es_model::recording::{InputAccess, PressPhase};
use es_record::input::{
    EventSource, HeldModifiers, InputBuilder, InputCollector, RawEvent, access, is_event_node,
};
use es_record::{Clock, RecordingOutput, SystemClock, movie_time};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIRST: u64 = 1_000_000;
const ESC: u16 = 1;
const KEY_1: u16 = 2;
const BACKSPACE: u16 = 14;
const TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_P: u16 = 25;
const ENTER: u16 = 28;
const CTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_L: u16 = 38;
const SHIFT: u16 = 42;
const KEY_C: u16 = 46;
const KEY_T: u16 = 20;
const COMMA: u16 = 51;
const ALT: u16 = 56;
const SPACE: u16 = 57;
const F5: u16 = 63;
const KP_7: u16 = 71;
const RIGHT_ALT: u16 = 100;
const UP: u16 = 103;
const LEFT: u16 = 105;
const SUPER: u16 = 125;

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
    RawEvent::Key { code, down, t_us }
}

fn button(down: bool, t_us: u64) -> RawEvent {
    RawEvent::Button {
        code: 0x110,
        down,
        t_us,
    }
}

fn tap(code: u16) -> [(u16, bool); 2] {
    [(code, true), (code, false)]
}

fn chord(mods: &[u16], code: u16) -> Vec<(u16, bool)> {
    let mut seq: Vec<_> = mods.iter().map(|&m| (m, true)).collect();
    seq.extend(tap(code));
    seq.extend(mods.iter().rev().map(|&m| (m, false)));
    seq
}

fn recorded(seq: &[(u16, bool)]) -> Vec<String> {
    let mut b = InputBuilder::new(rect());
    for (n, &(code, down)) in seq.iter().enumerate() {
        b.event(key(code, down, FIRST + 1000 * (n as u64 + 1)), None);
    }
    b.finish(&out(&[(FIRST, FIRST + 10_000_000)]))
        .keystrokes
        .iter()
        .map(|k| format!("{}{}", k.modifiers.concat(), k.key))
        .collect()
}

#[test]
fn plain_typing_and_shifted_characters_are_never_stored() {
    let mut password = Vec::new();
    for code in [KEY_P, KEY_A, KEY_1, COMMA, SPACE, KP_7] {
        password.extend(tap(code));
    }
    password.extend(chord(&[SHIFT], KEY_Q));
    password.extend(chord(&[SHIFT], KEY_1));
    password.extend(chord(&[RIGHT_ALT], KEY_Q));
    password.extend(chord(&[RIGHT_ALT, SHIFT], KEY_A));
    assert_eq!(recorded(&password), Vec::<String>::new());
}

#[test]
fn shortcuts_and_special_keys_are_stored() {
    let mut seq = Vec::new();
    seq.extend(chord(&[CTRL], KEY_C));
    seq.extend(chord(&[ALT], TAB));
    seq.extend(chord(&[SUPER], KEY_L));
    seq.extend(chord(&[CTRL, SHIFT], KEY_T));
    for code in [ENTER, ESC, UP, LEFT, F5, BACKSPACE] {
        seq.extend(tap(code));
    }
    seq.extend(chord(&[SHIFT], TAB));
    assert_eq!(
        recorded(&seq),
        [
            "⌃C",
            "⌥Tab",
            "⌘L",
            "⌃⇧T",
            "Enter",
            "Esc",
            "Up",
            "Left",
            "F5",
            "Backspace",
            "⇧Tab"
        ]
    );
}

#[test]
fn a_modifier_whose_release_was_lost_stops_counting_once_the_device_says_it_is_up() {
    let mut held = HeldModifiers::default();
    let mut b = InputBuilder::new(rect());
    let mut feed = |b: &mut InputBuilder, ev: RawEvent| {
        if let RawEvent::Key { code, down, .. } = ev {
            held.note(code, down);
        }
        b.event(ev, None);
    };
    feed(&mut b, key(CTRL, true, FIRST + 1));
    feed(&mut b, key(KEY_C, true, FIRST + 2));
    for ev in held.lost(|_| false, FIRST + 3) {
        b.event(ev, None);
    }
    assert!(held.is_empty());
    b.event(key(KEY_A, true, FIRST + 4), None);
    b.event(key(KEY_A, false, FIRST + 5), None);
    let keys = b.finish(&out(&[(FIRST, FIRST + 1_000_000)])).keystrokes;
    assert_eq!(keys.len(), 1, "only the real Ctrl+C: {keys:?}");
    assert_eq!(keys[0].key, "C");

    let mut still = HeldModifiers::default();
    still.note(CTRL, true);
    still.note(KEY_A, true);
    assert!(still.lost(|c| c == CTRL, 0).is_empty());
    assert!(!still.is_empty());
}

#[test]
fn presses_carry_the_pointer_normalised_and_no_position_when_unknown() {
    let mut b = InputBuilder::new(rect());
    b.event(
        button(true, FIRST + 200_000),
        Some(Point::new(200.0, 300.0)),
    );
    b.event(button(false, FIRST + 400_000), Some(Point::new(0.0, 900.0)));
    b.event(button(true, FIRST + 500_000), None);
    b.event(
        RawEvent::Button {
            code: 0x14a,
            down: true,
            t_us: FIRST + 600_000,
        },
        None,
    );
    let i = b.finish(&out(&[(FIRST, FIRST + 9_000_000)]));
    assert!(i.travel.is_empty());
    assert_eq!(i.presses.len(), 3, "a touch contact is not a click");
    let p = &i.presses;
    assert_eq!((p[0].x, p[0].y, p[0].button), (Some(0.5), Some(0.5), 0));
    assert_eq!(p[0].phase, PressPhase::Down);
    assert_eq!((p[1].x, p[1].y), (Some(0.0), Some(1.0)));
    assert_eq!(p[1].phase, PressPhase::Up);
    assert_eq!((p[2].x, p[2].y), (None, None));
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

    let mut b = InputBuilder::new(rect());
    b.event(button(true, FIRST + 100_000), None);
    b.event(key(ENTER, true, FIRST + 2_000_000), None);
    b.event(key(ESC, true, FIRST + 3_500_000), None);
    let i = b.finish(&o);
    assert_eq!(i.presses.iter().map(|p| p.time).collect::<Vec<_>>(), [0.1]);
    let keys: Vec<_> = i.keystrokes.iter().map(|k| (k.time, &*k.key)).collect();
    assert_eq!(keys, [(1.5, "Esc")]);
}

#[test]
fn input_json_has_the_documented_shape() {
    let mut b = InputBuilder::new(rect());
    b.event(
        button(true, FIRST + 200_000),
        Some(Point::new(200.0, 300.0)),
    );
    b.event(button(false, FIRST + 300_000), None);
    b.event(key(CTRL, true, FIRST + 400_000), None);
    b.event(key(KEY_C, true, FIRST + 500_000), None);
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

struct Scripted(Arc<Mutex<Vec<RawEvent>>>);
impl EventSource for Scripted {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        std::thread::sleep(wait);
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[test]
fn the_collector_reads_a_fake_source_and_asks_for_the_pointer_only_on_clicks() {
    let clock = Arc::new(SystemClock::default());
    let queue = Arc::new(Mutex::new(Vec::new()));
    let samples = Arc::new(Mutex::new(0));
    let c = InputCollector::start(
        rect(),
        Some(Arc::new(Still(samples.clone()))),
        Box::new(Scripted(queue.clone())),
    );
    let first = clock.monotonic_us();
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(*samples.lock().unwrap(), 0, "no pointer polling while idle");
    queue.lock().unwrap().extend([
        button(true, clock.monotonic_us()),
        key(CTRL, true, clock.monotonic_us()),
        key(KEY_C, true, clock.monotonic_us()),
        key(KEY_A, true, clock.monotonic_us()),
    ]);
    std::thread::sleep(Duration::from_millis(80));
    let end = clock.monotonic_us();
    let i = c.stop().unwrap().finish(&RecordingOutput {
        file: PathBuf::new(),
        first_frame_monotonic_us: first,
        segments: vec![(first, end)],
    });
    assert!(i.travel.is_empty());
    assert_eq!(i.presses.len(), 1);
    assert_eq!(i.presses[0].x, Some(0.5));
    assert_eq!(i.keystrokes.len(), 2);
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
