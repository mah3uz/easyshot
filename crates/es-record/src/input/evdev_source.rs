use super::{EventSource, HeldKeys, RawEvent, event_nodes};
use crate::Clock;
use evdev::{Device, EventType, KeyCode};
use std::os::fd::OwnedFd;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Read-only, so no descriptor this process holds can write events into a device.
fn open_read_only(path: &Path) -> Option<Device> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)
        .ok()?;
    Device::from_fd(OwnedFd::from(file)).ok()
}

/// Opens every readable `eventN` node under `dir` to read its capabilities, so it touches real
/// devices; callers outside tests only.
pub fn discover_devices(dir: &Path) -> Vec<PathBuf> {
    event_nodes(dir)
        .into_iter()
        .filter(|p| {
            open_read_only(p).is_some_and(|d| {
                d.supported_keys()
                    .is_some_and(|k| k.contains(KeyCode::BTN_LEFT) || k.contains(KeyCode::KEY_A))
            })
        })
        .collect()
}

struct Node {
    device: Device,
    id: u32,
    held: HeldKeys,
}

pub struct EvdevSource {
    nodes: Vec<Node>,
    clock: Arc<dyn Clock>,
}

fn realtime_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64)
}

impl EvdevSource {
    pub fn open(paths: &[PathBuf], clock: Arc<dyn Clock>) -> Self {
        let nodes = paths
            .iter()
            .filter_map(|p| {
                let device = open_read_only(p)?;
                device.set_nonblocking(true).ok()?;
                Some(device)
            })
            .zip(0..)
            .map(|(device, id)| Node {
                device,
                id,
                held: HeldKeys::default(),
            })
            .collect();
        Self { nodes, clock }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Node {
    /// False when the device is gone; every key it held is released then.
    fn read(&mut self, delta: i128, now: u64, out: &mut Vec<RawEvent>) -> bool {
        let id = self.id;
        match self.device.fetch_events() {
            Ok(events) => {
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
                        self.held.note(code, down);
                        RawEvent::Key {
                            device: id,
                            code,
                            down,
                            t_us,
                        }
                    });
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                out.extend(self.held.lost(id, |_| false, now));
                return false;
            }
        }
        if self.held.is_empty() {
            return true;
        }
        match self.device.get_key_state() {
            Ok(keys) => {
                out.extend(self.held.lost(id, |c| keys.contains(KeyCode::new(c)), now));
                true
            }
            Err(_) => {
                out.extend(self.held.lost(id, |_| false, now));
                false
            }
        }
    }
}

impl EventSource for EvdevSource {
    fn next_batch(&mut self, wait: Duration) -> Vec<RawEvent> {
        std::thread::sleep(wait);
        let now = self.clock.monotonic_us();
        let delta = i128::from(realtime_us()) - i128::from(now);
        let mut out = Vec::new();
        self.nodes
            .retain_mut(|node| node.read(delta, now, &mut out));
        out
    }
}
