use super::ipc::Ipc;
use super::json::{
    Client, Monitor, address, all_windows, focus_command, join_toplevels, visible_windows,
    window_info,
};
use crate::error::{Error, Result};
use crate::traits::WindowList;
use es_model::platform::{WindowEvent, WindowId, WindowInfo};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const EVENT_BUFFER: usize = 256;
const TOPLEVEL_RETRIES: u32 = 3;
const TOPLEVEL_WAIT: Duration = Duration::from_millis(20);

pub type Toplevels = Arc<dyn Fn() -> Vec<WindowInfo> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprEvent {
    Opened(String),
    Closed(String),
    Focused(String),
    Moved(String),
}

pub fn parse_event(line: &str) -> Option<HyprEvent> {
    let (name, data) = line.trim_end().split_once(">>")?;
    let first = data.split(',').next().filter(|a| !a.is_empty());
    let addr = || first.map(address);
    match name {
        "openwindow" => addr().map(HyprEvent::Opened),
        "closewindow" => addr().map(HyprEvent::Closed),
        "activewindowv2" => addr().map(HyprEvent::Focused),
        "movewindowv2" => addr().map(HyprEvent::Moved),
        _ => None,
    }
}

struct Inner {
    ipc: Ipc,
    toplevels: Option<Toplevels>,
    // Once given, an id never changes, so events and snapshots agree.
    ids: Mutex<HashMap<String, WindowId>>,
}

impl Inner {
    fn fetch(&self) -> Result<(Vec<Client>, Vec<Monitor>)> {
        Ok((self.ipc.json("clients")?, self.ipc.json("monitors")?))
    }

    fn assign(&self, windows: &mut [WindowInfo], clients: &[Client], wait: bool) {
        let unknown: Vec<String> = {
            let ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
            windows
                .iter()
                .filter(|w| !ids.contains_key(&w.id.0))
                .map(|w| w.id.0.clone())
                .collect()
        };
        let mut joined = HashMap::new();
        if let Some(source) = &self.toplevels
            && !unknown.is_empty()
        {
            // The foreign-toplevel announcement can trail `openwindow` by a frame.
            for attempt in 0..=TOPLEVEL_RETRIES {
                joined = join_toplevels(clients, &source());
                if !wait || unknown.iter().all(|a| joined.contains_key(a)) {
                    break;
                }
                if attempt < TOPLEVEL_RETRIES {
                    std::thread::sleep(TOPLEVEL_WAIT);
                }
            }
        }
        let mut ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
        for w in windows {
            let addr = w.id.0.clone();
            let id = ids
                .entry(addr.clone())
                .or_insert_with(|| joined.remove(&addr).unwrap_or_else(|| w.id.clone()));
            w.id = id.clone();
        }
    }

    fn address_of(&self, id: &WindowId) -> String {
        let ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
        ids.iter()
            .find(|(_, known)| *known == id)
            .map_or_else(|| id.0.clone(), |(addr, _)| addr.clone())
    }

    fn known(&self, addr: &str) -> WindowId {
        let ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
        ids.get(addr)
            .cloned()
            .unwrap_or_else(|| WindowId(addr.to_owned()))
    }

    fn forget(&self, addr: &str) -> WindowId {
        let mut ids = self.ids.lock().unwrap_or_else(|e| e.into_inner());
        ids.remove(addr)
            .unwrap_or_else(|| WindowId(addr.to_owned()))
    }

    fn one(&self, addr: &str) -> Result<WindowInfo> {
        let (clients, monitors) = self.fetch()?;
        let client = clients
            .iter()
            .find(|c| address(&c.address) == addr)
            .ok_or_else(|| Error::NotFound(addr.to_owned()))?;
        let mut info = [window_info(client, &monitors)];
        self.assign(&mut info, &clients, true);
        let [info] = info;
        Ok(info)
    }

    fn translate(&self, e: HyprEvent) -> Option<WindowEvent> {
        let lookup = |addr: &str| {
            self.one(addr)
                .inspect_err(|err| tracing::debug!("hyprland: window {addr}: {err}"))
                .ok()
        };
        match e {
            HyprEvent::Opened(a) => lookup(&a).map(WindowEvent::Opened),
            HyprEvent::Moved(a) => lookup(&a).map(WindowEvent::Changed),
            HyprEvent::Closed(a) => Some(WindowEvent::Closed(self.forget(&a))),
            HyprEvent::Focused(a) => Some(WindowEvent::Focused(self.known(&a))),
        }
    }
}

pub struct HyprWindows {
    inner: Arc<Inner>,
    events: async_channel::Receiver<WindowEvent>,
}

impl HyprWindows {
    /// `toplevels` lists foreign toplevels, whose identifiers window capture needs.
    pub fn start(ipc: Ipc, toplevels: Option<Toplevels>) -> Result<Self> {
        let stream = ipc.connect_events()?;
        let inner = Arc::new(Inner {
            ipc,
            toplevels,
            ids: Mutex::new(HashMap::new()),
        });
        let (tx, events) = async_channel::bounded(EVENT_BUFFER);
        let reader = inner.clone();
        std::thread::Builder::new()
            .name("hyprland-events".into())
            .spawn(move || read_events(stream, &reader, &tx))
            .map_err(|source| Error::Io {
                path: "hyprland-events".into(),
                source,
            })?;
        Ok(HyprWindows { inner, events })
    }

    pub fn try_snapshot(&self) -> Result<Vec<WindowInfo>> {
        let (clients, monitors) = self.inner.fetch()?;
        let mut windows = visible_windows(&clients, &monitors);
        self.inner.assign(&mut windows, &clients, false);
        Ok(windows)
    }

    pub fn try_all(&self) -> Result<Vec<WindowInfo>> {
        let (clients, monitors) = self.inner.fetch()?;
        let mut windows = all_windows(&clients, &monitors);
        self.inner.assign(&mut windows, &clients, false);
        Ok(windows)
    }
}

fn read_events(stream: UnixStream, inner: &Inner, tx: &async_channel::Sender<WindowEvent>) {
    for line in BufReader::new(stream).lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("hyprland: event socket: {e}");
                return;
            }
        };
        let Some(event) = parse_event(&line).and_then(|e| inner.translate(e)) else {
            continue;
        };
        match tx.try_send(event) {
            Ok(()) => {}
            Err(async_channel::TrySendError::Full(_)) => {
                tracing::debug!("hyprland: window event dropped, nobody is reading");
            }
            Err(async_channel::TrySendError::Closed(_)) => return,
        }
    }
    tracing::info!("hyprland: event socket closed");
}

impl WindowList for HyprWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        self.try_snapshot()
            .inspect_err(|e| tracing::warn!("hyprland: window list: {e}"))
            .unwrap_or_default()
    }
    fn has_geometry(&self) -> bool {
        true
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.events.clone()
    }
    fn all(&self) -> Vec<WindowInfo> {
        self.try_all()
            .inspect_err(|e| tracing::warn!("hyprland: window list: {e}"))
            .unwrap_or_default()
    }
    fn can_focus(&self) -> bool {
        true
    }
    fn focus(&self, w: &WindowId) -> Result<()> {
        let reply = self
            .inner
            .ipc
            .request(&focus_command(&self.inner.address_of(w)))?;
        match String::from_utf8_lossy(&reply).trim() {
            "ok" => Ok(()),
            other => Err(Error::Protocol(format!("hyprland focuswindow: {other}"))),
        }
    }
}
