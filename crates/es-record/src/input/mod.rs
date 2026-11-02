mod keys;

#[cfg(feature = "evdev-input")]
mod evdev_source;

#[cfg(feature = "evdev-input")]
pub use evdev_source::{EvdevSource, discover_devices, is_event_node};
pub use keys::ModState;

use crate::{Clock, Error, RecordingOutput, Result};
use es_geom::{Point, Rect};
use es_model::platform::CursorSource;
use es_model::recording::{
    KeystrokeSample, PointerSample, PressPhase, PressSample, RecordingInput, TravelKind,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

const BTN_FIRST: u16 = 0x110;
const BTN_LAST: u16 = 0x11f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawEvent {
    Button { code: u16, down: bool, t_us: u64 },
    Key { code: u16, down: bool, t_us: u64 },
}

pub trait EventSource: Send {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent>;
}

#[derive(Debug, Clone, Copy)]
pub struct InputConfig {
    pub capture_rect: Rect,
    pub keystrokes: bool,
}

pub struct InputBuilder {
    cfg: InputConfig,
    travel: Vec<(u64, Point, bool)>,
    presses: Vec<(u64, Point, u32, PressPhase)>,
    keys: Vec<(u64, Vec<String>, String)>,
    held: u32,
    mods: ModState,
    last: Option<Point>,
}

impl InputBuilder {
    pub fn new(cfg: InputConfig) -> Self {
        Self {
            cfg,
            travel: Vec::new(),
            presses: Vec::new(),
            keys: Vec::new(),
            held: 0,
            mods: ModState::default(),
            last: None,
        }
    }

    pub fn cursor(&mut self, p: Point, t_us: u64) {
        if self.last == Some(p) {
            return;
        }
        self.last = Some(p);
        self.travel.push((t_us, p, self.held > 0));
    }

    pub fn event(&mut self, ev: RawEvent) {
        match ev {
            RawEvent::Button { code, down, t_us } => {
                if !(BTN_FIRST..=BTN_LAST).contains(&code) {
                    return;
                }
                self.held = if down {
                    self.held.saturating_add(1)
                } else {
                    self.held.saturating_sub(1)
                };
                let Some(p) = self.last else { return };
                let phase = if down {
                    PressPhase::Down
                } else {
                    PressPhase::Up
                };
                self.presses
                    .push((t_us, p, u32::from(code - BTN_FIRST), phase));
            }
            RawEvent::Key { code, down, t_us } => {
                let hit = self.mods.feed(code, down);
                if let (true, Some((mods, key))) = (self.cfg.keystrokes, hit) {
                    tracing::debug!(?mods, key, "keystroke");
                    self.keys.push((t_us, mods, key));
                }
            }
        }
    }

    pub fn finish(self, out: &RecordingOutput) -> RecordingInput {
        let r = self.cfg.capture_rect;
        let (w, h) = (r.width().max(1.0), r.height().max(1.0));
        let norm = |p: Point| {
            (
                ((p.x - r.x0) / w).clamp(0.0, 1.0),
                ((p.y - r.y0) / h).clamp(0.0, 1.0),
            )
        };
        let at = |t| movie_time(t, out);
        let mut travel: Vec<PointerSample> = self
            .travel
            .iter()
            .filter_map(|&(t, p, drag)| {
                let (x, y) = norm(p);
                Some(PointerSample {
                    time: at(t)?,
                    x,
                    y,
                    kind: if drag {
                        TravelKind::Drag
                    } else {
                        TravelKind::Move
                    },
                })
            })
            .collect();
        let mut presses: Vec<PressSample> = self
            .presses
            .iter()
            .filter_map(|&(t, p, button, phase)| {
                let (x, y) = norm(p);
                Some(PressSample {
                    time: at(t)?,
                    x,
                    y,
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
        travel.sort_by(|a, b| a.time.total_cmp(&b.time));
        presses.sort_by(|a, b| a.time.total_cmp(&b.time));
        keystrokes.sort_by(|a, b| a.time.total_cmp(&b.time));
        RecordingInput {
            travel,
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
    pub fn start(
        cfg: InputConfig,
        cursor: Arc<dyn CursorSource>,
        clock: Arc<dyn Clock>,
        mut events: Option<Box<dyn EventSource>>,
    ) -> Self {
        let builder = Arc::new(Mutex::new(InputBuilder::new(cfg)));
        let quit = Arc::new(AtomicBool::new(false));
        let (b, q) = (builder.clone(), quit.clone());
        let tick = Duration::from_millis(16);
        let thread = std::thread::spawn(move || {
            while !q.load(Ordering::Relaxed) {
                let batch = match events.as_mut() {
                    Some(src) => src.next_batch(tick),
                    None => {
                        clock.sleep(tick);
                        Vec::new()
                    }
                };
                let now = clock.monotonic_us();
                let pos = cursor.sample().map(|(p, _)| p);
                let Ok(mut g) = b.lock() else { return };
                if let Some(p) = pos {
                    g.cursor(p, now);
                }
                for ev in batch {
                    g.event(ev);
                }
            }
        });
        Self {
            builder,
            quit,
            thread: Some(thread),
        }
    }

    pub fn finish(mut self, out: &RecordingOutput) -> Result<RecordingInput> {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let mut g = self
            .builder
            .lock()
            .map_err(|_| Error::Protocol("input collector poisoned".into()))?;
        let cfg = g.cfg;
        let b = std::mem::replace(&mut *g, InputBuilder::new(cfg));
        drop(g);
        Ok(b.finish(out))
    }
}

impl Drop for InputCollector {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
