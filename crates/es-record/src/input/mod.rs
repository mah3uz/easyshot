mod keys;

#[cfg(feature = "evdev-input")]
mod evdev_source;

#[cfg(feature = "evdev-input")]
pub use evdev_source::{EvdevSource, discover_devices};
pub use keys::{HeldModifiers, ModState};

use crate::{Clock, Error, RecordingOutput, Result};
use es_geom::{Point, Rect};
use es_model::platform::CursorSource;
use es_model::recording::{InputAccess, KeystrokeSample, PressPhase, PressSample, RecordingInput};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

pub const DEV_INPUT: &str = "/dev/input";

const BTN_FIRST: u16 = 0x110;
const BTN_LAST: u16 = 0x11f;
const TICK: Duration = Duration::from_millis(16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawEvent {
    Button { code: u16, down: bool, t_us: u64 },
    Key { code: u16, down: bool, t_us: u64 },
}

pub trait EventSource: Send {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent>;
}

pub fn is_event_node(name: &str) -> bool {
    name.strip_prefix("event")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn event_nodes(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(is_event_node))
        .map(|e| e.path())
        .collect();
    nodes.sort();
    nodes
}

/// Granted only when every event node under `dir` is readable, which is what membership of the
/// `input` group gives. Checks permissions without opening any device.
pub fn access(dir: &Path) -> InputAccess {
    if !cfg!(feature = "evdev-input") {
        return InputAccess::NotBuilt;
    }
    let nodes = event_nodes(dir);
    let readable = |p: &PathBuf| rustix::fs::access(p, rustix::fs::Access::READ_OK).is_ok();
    if !nodes.is_empty() && nodes.iter().all(readable) {
        InputAccess::Granted
    } else {
        InputAccess::Denied
    }
}

/// Opens every keyboard and mouse under [`DEV_INPUT`]; touches real devices, so never in tests.
#[cfg(feature = "evdev-input")]
pub fn open_system(
    clock: Arc<dyn Clock>,
) -> std::result::Result<Box<dyn EventSource>, InputAccess> {
    let dir = Path::new(DEV_INPUT);
    match access(dir) {
        InputAccess::Granted => {}
        other => return Err(other),
    }
    let source = EvdevSource::open(&discover_devices(dir), clock);
    if source.is_empty() {
        return Err(InputAccess::Denied);
    }
    Ok(Box::new(source))
}

#[cfg(not(feature = "evdev-input"))]
pub fn open_system(
    _clock: Arc<dyn Clock>,
) -> std::result::Result<Box<dyn EventSource>, InputAccess> {
    Err(InputAccess::NotBuilt)
}

pub struct InputBuilder {
    capture_rect: Rect,
    presses: Vec<(u64, Option<Point>, u32, PressPhase)>,
    keys: Vec<(u64, Vec<String>, String)>,
    mods: ModState,
}

impl InputBuilder {
    pub fn new(capture_rect: Rect) -> Self {
        Self {
            capture_rect,
            presses: Vec::new(),
            keys: Vec::new(),
            mods: ModState::default(),
        }
    }

    /// `pointer` is where the pointer was when the event was read, in the capture rect's space.
    pub fn event(&mut self, ev: RawEvent, pointer: Option<Point>) {
        match ev {
            RawEvent::Button { code, down, t_us } => {
                if !(BTN_FIRST..=BTN_LAST).contains(&code) {
                    return;
                }
                let phase = if down {
                    PressPhase::Down
                } else {
                    PressPhase::Up
                };
                self.presses
                    .push((t_us, pointer, u32::from(code - BTN_FIRST), phase));
            }
            RawEvent::Key { code, down, t_us } => {
                if let Some((mods, key)) = self.mods.feed(code, down) {
                    self.keys.push((t_us, mods, key));
                }
            }
        }
    }

    pub fn finish(self, out: &RecordingOutput) -> RecordingInput {
        let r = self.capture_rect;
        let (w, h) = (r.width().max(1.0), r.height().max(1.0));
        let at = |t| movie_time(t, out);
        let mut presses: Vec<PressSample> = self
            .presses
            .iter()
            .filter_map(|&(t, p, button, phase)| {
                Some(PressSample {
                    time: at(t)?,
                    x: p.map(|p| ((p.x - r.x0) / w).clamp(0.0, 1.0)),
                    y: p.map(|p| ((p.y - r.y0) / h).clamp(0.0, 1.0)),
                    button,
                    phase,
                })
            })
            .collect();
        let mut keystrokes: Vec<KeystrokeSample> = self
            .keys
            .into_iter()
            .filter_map(|(t, modifiers, key)| {
                Some(KeystrokeSample {
                    time: at(t)?,
                    modifiers,
                    key,
                })
            })
            .collect();
        presses.sort_by(|a, b| a.time.total_cmp(&b.time));
        keystrokes.sort_by(|a, b| a.time.total_cmp(&b.time));
        RecordingInput {
            travel: Vec::new(),
            presses,
            keystrokes,
            is_sanitized: false,
        }
    }
}

/// Seconds on the movie timeline for a monotonic stamp, with paused spans removed;
/// `None` when the stamp falls before the first frame or inside a pause.
pub fn movie_time(t_us: u64, out: &RecordingOutput) -> Option<f64> {
    let first = out.first_frame_monotonic_us;
    if t_us < first {
        return None;
    }
    if out.segments.is_empty() {
        return Some((t_us - first) as f64 / 1e6);
    }
    let mut paused = 0u64;
    let mut prev_end = None;
    for &(start, end) in &out.segments {
        if let Some(e) = prev_end {
            paused += start.saturating_sub(e);
        }
        if t_us < start {
            return None;
        }
        if t_us <= end {
            return Some((t_us - first - paused) as f64 / 1e6);
        }
        prev_end = Some(end);
    }
    None
}

pub fn write_input(path: &Path, input: &RecordingInput) -> Result<()> {
    crate::project::write_json(path, input)
}

pub struct InputCollector {
    builder: Arc<Mutex<InputBuilder>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl InputCollector {
    /// Reads `events` on its own thread; the pointer is asked for only when a button changes.
    pub fn start(
        capture_rect: Rect,
        cursor: Option<Arc<dyn CursorSource>>,
        mut events: Box<dyn EventSource>,
    ) -> Self {
        let builder = Arc::new(Mutex::new(InputBuilder::new(capture_rect)));
        let quit = Arc::new(AtomicBool::new(false));
        let (b, q) = (builder.clone(), quit.clone());
        let thread = std::thread::spawn(move || {
            while !q.load(Ordering::Relaxed) {
                let batch = events.next_batch(TICK);
                let pointer = batch
                    .iter()
                    .any(|e| matches!(e, RawEvent::Button { .. }))
                    .then(|| cursor.as_ref().and_then(|c| c.sample()))
                    .flatten()
                    .map(|(p, _)| p);
                let Ok(mut g) = b.lock() else { return };
                for ev in batch {
                    g.event(ev, pointer);
                }
            }
        });
        Self {
            builder,
            quit,
            thread: Some(thread),
        }
    }

    /// Stops reading the devices and returns what was read.
    pub fn stop(mut self) -> Result<InputBuilder> {
        self.join();
        let mut g = self
            .builder
            .lock()
            .map_err(|_| Error::Protocol("input collector poisoned".into()))?;
        let rect = g.capture_rect;
        Ok(std::mem::replace(&mut *g, InputBuilder::new(rect)))
    }

    fn join(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for InputCollector {
    fn drop(&mut self) {
        self.join();
    }
}
