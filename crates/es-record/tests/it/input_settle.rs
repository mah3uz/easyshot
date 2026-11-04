#![allow(clippy::unwrap_used)]

use crate::input_keys::keymap_text;
use es_geom::Rect;
use es_model::platform::{KeymapSource, KeymapText};
use es_record::RecordingOutput;
use es_record::input::{EventSource, InputCollector, RawEvent, Sources};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ESC: u16 = 1;
const KEY_Q: u16 = 16;
const ENTER: u16 = 28;
const CTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_C: u16 = 46;
const RIGHT_CTRL: u16 = 97;

#[derive(Default)]
struct Published(Mutex<Option<KeymapText>>);
impl KeymapSource for Published {
    fn keymap(&self) -> Option<KeymapText> {
        self.0.lock().unwrap().clone()
    }
}

impl Published {
    fn set(&self, serial: u64, options: &str) {
        *self.0.lock().unwrap() = Some(KeymapText {
            serial,
            text: keymap_text("us", options).into(),
        });
    }
}

type Step = Box<dyn FnOnce() -> Vec<RawEvent> + Send>;

/// Hands out one batch per tick, at once, so what happens between two reads is exact.
struct Stepped {
    steps: VecDeque<Step>,
    done: Arc<AtomicBool>,
}

impl EventSource for Stepped {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        match self.steps.pop_front() {
            Some(step) => step(),
            None => {
                self.done.store(true, Ordering::SeqCst);
                std::thread::sleep(wait);
                Vec::new()
            }
        }
    }
}

fn keys(device: u32, seq: &[(u16, bool)]) -> Vec<RawEvent> {
    seq.iter()
        .zip(1u64..)
        .map(|(&(code, down), t)| RawEvent::Key {
            device,
            code,
            down,
            t_us: t * 1000,
        })
        .collect()
}

fn chord(modifier: u16, key: u16) -> [(u16, bool); 4] {
    [
        (modifier, true),
        (key, true),
        (key, false),
        (modifier, false),
    ]
}

fn run(open: Arc<AtomicBool>, keymap: Arc<Published>, steps: Vec<Step>) -> Vec<String> {
    let done = Arc::new(AtomicBool::new(false));
    let source = Stepped {
        steps: steps.into(),
        done: done.clone(),
    };
    let sources = Sources {
        cursor: None,
        keymap: Some(keymap),
        session_open: open,
    };
    let c = InputCollector::start(Rect::new(0.0, 0.0, 10.0, 10.0), sources, Box::new(source));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "the steps never ran");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(60));
    let out = RecordingOutput {
        file: PathBuf::new(),
        first_frame_monotonic_us: 0,
        segments: Vec::new(),
        split_audio: false,
    };
    c.stop()
        .unwrap()
        .finish(&out)
        .keystrokes
        .into_iter()
        .map(|k| format!("{}{}", k.modifiers.concat(), k.key))
        .collect()
}

fn idle() -> Step {
    Box::new(Vec::new)
}

// The laptop's Right Ctrl is AltGr (lv3:switch), so Right Ctrl + Q types "@"; read with the
// external keyboard's keymap it would be stored as Ctrl+Q.
#[test]
fn keys_read_around_a_keymap_change_are_dropped_whichever_arrives_first() {
    let laptop_after_external = Arc::new(Published::default());
    laptop_after_external.set(1, "");
    let km = laptop_after_external.clone();
    let late = run(
        Arc::new(AtomicBool::new(true)),
        laptop_after_external,
        vec![
            idle(),
            Box::new(|| keys(1, &chord(RIGHT_CTRL, KEY_Q))),
            Box::new(move || {
                km.set(2, "lv3:switch");
                Vec::new()
            }),
            idle(),
            Box::new(|| keys(1, &chord(CTRL, KEY_C))),
        ],
    );
    assert_eq!(late, ["⌃C"], "the keymap arrived a tick after the keys");

    let external_after_laptop = Arc::new(Published::default());
    external_after_laptop.set(1, "lv3:switch");
    let km = external_after_laptop.clone();
    let early = run(
        Arc::new(AtomicBool::new(true)),
        external_after_laptop,
        vec![
            idle(),
            Box::new(move || {
                km.set(2, "");
                let mut batch = keys(1, &chord(RIGHT_CTRL, KEY_Q));
                batch.extend(keys(0, &[(KEY_A, true), (KEY_A, false)]));
                batch
            }),
            idle(),
            idle(),
            Box::new(|| keys(0, &chord(CTRL, KEY_C))),
        ],
    );
    assert_eq!(
        early,
        ["⌃C"],
        "the keymap arrived before the keys were read"
    );
}

// An unlock can reach this process within the tick that read the Enter which caused it.
#[test]
fn keys_read_while_locked_are_dropped_even_when_the_unlock_lands_in_the_same_tick() {
    let open = Arc::new(AtomicBool::new(true));
    let keymap = Arc::new(Published::default());
    keymap.set(1, "");
    let (lock, unlock) = (open.clone(), open.clone());
    let stored = run(
        open,
        keymap,
        vec![
            idle(),
            Box::new(move || {
                lock.store(false, Ordering::SeqCst);
                Vec::new()
            }),
            Box::new(move || {
                unlock.store(true, Ordering::SeqCst);
                keys(0, &[(ENTER, true), (ENTER, false)])
            }),
            idle(),
            Box::new(|| keys(0, &[(ESC, true), (ESC, false)])),
        ],
    );
    assert_eq!(stored, ["Esc"]);
}
