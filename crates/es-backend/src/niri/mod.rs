mod binds;
mod ipc;
mod layout;
mod wire;

use crate::error::{BackendError, Error, Result};
use crate::include_scan;
use crate::probe::{Probe, Provider};
use crate::traits::{BindHelper, BindSyntax, PointerQuery, Trigger, WindowList};
use es_geom::Point;
use es_model::platform::{OutputId, WindowEvent, WindowInfo};
use es_protocol::Route;
use ipc::Ipc;
use layout::State;
use std::collections::HashMap;
use std::io::BufReader;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use wire::{Event, Output, Request, Response};

pub const SOCKET_ENV: &str = "NIRI_SOCKET";

fn detect(p: &Probe) -> Option<u8> {
    p.env.var(SOCKET_ENV)?;
    let named = p
        .env
        .current_desktop()
        .is_some_and(|d| d.split(':').any(|n| n.eq_ignore_ascii_case("niri")));
    Some(if named { 30 } else { 25 })
}

fn ipc(p: &Probe) -> std::result::Result<Ipc, BackendError> {
    p.env
        .var(SOCKET_ENV)
        .map(Ipc::new)
        .ok_or_else(|| BackendError::Unavailable("the niri socket variable is not set".into()))
}

fn outputs(ipc: &Ipc) -> Result<HashMap<String, Output>> {
    match ipc.request(&Request::Outputs)? {
        Response::Outputs(o) => Ok(o),
        _ => Err(ipc::unexpected("Outputs")),
    }
}

fn initial_state(ipc: &Ipc) -> Result<State> {
    let Response::Workspaces(workspaces) = ipc.request(&Request::Workspaces)? else {
        return Err(ipc::unexpected("Workspaces"));
    };
    let Response::Windows(windows) = ipc.request(&Request::Windows)? else {
        return Err(ipc::unexpected("Windows"));
    };
    Ok(State {
        windows: windows.into_iter().map(|w| (w.id, w)).collect(),
        workspaces: workspaces.into_iter().map(|ws| (ws.id, ws)).collect(),
        outputs: outputs(ipc)?,
    })
}

struct NiriWindows {
    ipc: Ipc,
    state: Arc<Mutex<State>>,
    rx: async_channel::Receiver<WindowEvent>,
    stream: UnixStream,
}

impl NiriWindows {
    fn start(ipc: Ipc) -> Result<Self> {
        // The stream opens first so nothing between the baseline and it goes unseen.
        let (stream, reader) = ipc.event_stream()?;
        let state = Arc::new(Mutex::new(initial_state(&ipc)?));
        let (tx, rx) = async_channel::bounded(256);
        let (thread_ipc, thread_state) = (ipc.clone(), state.clone());
        std::thread::Builder::new()
            .name("easyshot-niri-events".into())
            .spawn(move || pump(&thread_ipc, reader, &thread_state, &tx))
            .map_err(|e| Error::Io {
                path: ipc.path.clone(),
                source: e,
            })?;
        Ok(NiriWindows {
            ipc,
            state,
            rx,
            stream,
        })
    }
}

fn pump(
    ipc: &Ipc,
    mut reader: BufReader<UnixStream>,
    state: &Mutex<State>,
    tx: &async_channel::Sender<WindowEvent>,
) {
    let mut line = Vec::new();
    loop {
        match ipc::next_line(&mut reader, &mut line) {
            Ok(true) => {}
            Ok(false) => {
                tracing::debug!("niri event stream ended");
                return;
            }
            Err(e) => {
                tracing::debug!("{e}");
                return;
            }
        }
        let event: Event = match ipc::parse(&line) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!("niri event skipped: {e}");
                continue;
            }
        };
        // Outputs come and go with their workspaces; niri sends no output event.
        let fresh_outputs = matches!(event, Event::WorkspacesChanged { .. })
            .then(|| outputs(ipc).inspect_err(|e| tracing::warn!("niri outputs: {e}")))
            .and_then(Result::ok);
        let events = {
            let mut st = state.lock().unwrap_or_else(PoisonError::into_inner);
            let before = st.infos();
            if let Some(o) = fresh_outputs {
                st.outputs = o;
            }
            let focused = st.apply(event);
            let mut events = layout::diff(&before, &st.infos());
            events.extend(focused.map(WindowEvent::Focused));
            events
        };
        for e in events {
            if tx.try_send(e).is_err() {
                tracing::debug!("niri window event dropped: no reader keeps up");
            }
        }
    }
}

impl Drop for NiriWindows {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl WindowList for NiriWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        let fresh = outputs(&self.ipc)
            .inspect_err(|e| tracing::warn!("niri outputs: {e}; using the last ones"))
            .ok();
        let st = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match fresh {
            // The state is the event reader's baseline, so newer outputs are applied to a copy.
            Some(outputs) => State {
                windows: st.windows.clone(),
                workspaces: st.workspaces.clone(),
                outputs,
            }
            .infos(),
            None => st.infos(),
        }
    }
    fn has_geometry(&self) -> bool {
        true
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.rx.clone()
    }
}

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "niri",
    probe: detect,
    build: |p| Ok(Box::new(NiriWindows::start(ipc(p)?)?)),
};

/// Niri has no pointer query, so the focused output stands in for the one under the pointer.
struct NiriPointer(Ipc);

impl NiriPointer {
    fn focused(&self) -> Result<Option<OutputId>> {
        match self.0.request(&Request::FocusedOutput)? {
            Response::FocusedOutput(o) => Ok(o.map(|o| OutputId(o.name))),
            _ => Err(ipc::unexpected("FocusedOutput")),
        }
    }
}

impl PointerQuery for NiriPointer {
    fn pointer_global(&self) -> Option<Point> {
        None
    }
    fn output_under_pointer(&self) -> Option<OutputId> {
        self.focused_output()
    }
    fn focused_output(&self) -> Option<OutputId> {
        self.focused()
            .inspect_err(|e| tracing::warn!("niri focused output: {e}"))
            .ok()
            .flatten()
    }
}

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "niri",
    probe: detect,
    build: |p| {
        let pointer = NiriPointer(ipc(p)?);
        pointer.focused()?;
        Ok(Box::new(pointer))
    },
};

struct NiriBinds {
    config: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl BindHelper for NiriBinds {
    fn syntax(&self) -> BindSyntax {
        BindSyntax::NiriKdl
    }
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String {
        binds::snippet(binds)
    }
    fn existing_binds(&self) -> Result<Vec<(Trigger, String)>> {
        let path = self
            .config
            .as_ref()
            .ok_or_else(|| Error::NotFound("niri config (no HOME)".into()))?;
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(binds::parse_config(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::NotFound(path.display().to_string()))
            }
            Err(source) => Err(Error::Io {
                path: path.clone(),
                source,
            }),
        }
    }
    fn file_name(&self) -> &'static str {
        "binds.kdl"
    }
    fn file_text(&self, binds: &[(Trigger, Route)]) -> String {
        binds::file(binds)
    }
    fn include_line(&self, dir: &str) -> String {
        format!("include optional=true \"{dir}/binds.kdl\"")
    }
    fn is_included(&self, dir: &Path) -> Result<bool> {
        let main = self
            .config
            .as_deref()
            .ok_or_else(|| Error::NotFound("niri config (no HOME)".into()))?;
        include_scan::scan(main, self.home.as_deref(), &include_scan::NIRI, dir)
    }
}

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "niri",
    probe: detect,
    build: |p| {
        Ok(Box::new(NiriBinds {
            config: binds::config_path(&p.env),
            home: include_scan::home(&p.env),
        }))
    },
};
