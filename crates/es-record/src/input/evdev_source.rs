use super::{EventSource, RawEvent};
use crate::Clock;
use evdev::{Device, EventType, KeyCode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn is_event_node(name: &str) -> bool {
    name.strip_prefix("event")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Opens every readable `eventN` node under `dir` to read its capabilities, so it touches real
/// devices; callers outside tests only.
pub fn discover_devices(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(is_event_node))
        .map(|e| e.path())
        .filter(|p| {
            Device::open(p).is_ok_and(|d| {
                d.supported_keys()
                    .is_some_and(|k| k.contains(KeyCode::BTN_LEFT) || k.contains(KeyCode::KEY_A))
            })
        })
        .collect();
    found.sort();
    found
}

pub struct EvdevSource {
    devices: Vec<Device>,
    clock: Arc<dyn Clock>,
}

fn realtime_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64)
}

impl EvdevSource {
    pub fn open(paths: &[PathBuf], clock: Arc<dyn Clock>) -> Self {
        let devices = paths
            .iter()
            .filter_map(|p| {
                let d = Device::open(p).ok()?;
                d.set_nonblocking(true).ok()?;
                Some(d)
            })
            .collect();
        Self { devices, clock }
    }

    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

impl EventSource for EvdevSource {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        std::thread::sleep(wait);
        let delta = i128::from(realtime_us()) - i128::from(self.clock.monotonic_us());
        let mut out = Vec::new();
        for dev in &mut self.devices {
            let Ok(events) = dev.fetch_events() else {
                continue;
            };
            for ev in events {
                if ev.event_type() != EventType::KEY || ev.value() > 1 {
                    continue;
                }
                let rt = ev
                    .timestamp()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_micros());
                let t_us = u64::try_from((rt as i128 - delta).max(0)).unwrap_or(0);
                let (code, down) = (ev.code(), ev.value() == 1);
                out.push(if (0x110..=0x11f).contains(&code) {
                    RawEvent::Button { code, down, t_us }
                } else {
                    RawEvent::Key { code, down, t_us }
                });
            }
        }
        out
    }
}
