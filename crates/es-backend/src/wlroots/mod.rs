mod binds;
mod ipc;
mod tree;

use crate::error::{BackendError, Error, Result};
use crate::include_scan;
use crate::probe::{Probe, Provider};
use crate::traits::{BindHelper, BindSyntax, PointerQuery, Trigger, WindowList};
use es_geom::Point;
use es_model::platform::{OutputId, WindowEvent, WindowInfo};
use es_protocol::Route;
use ipc::{EVENT_BIT, GET_CONFIG, GET_OUTPUTS, GET_TREE, Ipc, RUN_COMMAND};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

pub const SOCKET_ENV: &str = "SWAYSOCK";
const WINDOW_EVENT: u32 = EVENT_BIT | 3;

fn detect(p: &Probe) -> Option<u8> {
    p.env.var(SOCKET_ENV)?;
    let named = p
        .env
        .current_desktop()
        .is_some_and(|d| d.split(':').any(|n| n.eq_ignore_ascii_case("sway")));
    Some(if named { 30 } else { 25 })
}

fn ipc(p: &Probe) -> std::result::Result<Ipc, BackendError> {
    p.env
        .var(SOCKET_ENV)
        .map(Ipc::new)
        .ok_or_else(|| BackendError::Unavailable("the sway socket variable is not set".into()))
}

fn tree(ipc: &Ipc) -> Result<Vec<WindowInfo>> {
    Ok(tree::windows(&ipc.json::<tree::Node>(GET_TREE, b"")?))
}

struct SwayWindows {
    ipc: Ipc,
    cache: Arc<Mutex<Vec<WindowInfo>>>,
    rx: async_channel::Receiver<WindowEvent>,
    sub: UnixStream,
}

impl SwayWindows {
    fn start(ipc: Ipc) -> Result<Self> {
        // Subscribing first means nothing between the baseline and the stream goes unseen.
        let sub = ipc.subscribe(&["window", "workspace", "output"])?;
        let cache = Arc::new(Mutex::new(tree(&ipc)?));
        let reader = sub.try_clone().map_err(|e| Error::Io {
            path: ipc.path.clone(),
            source: e,
        })?;
        let (tx, rx) = async_channel::bounded(256);
        let (thread_ipc, thread_cache) = (ipc.clone(), cache.clone());
        std::thread::Builder::new()
            .name("easyshot-sway-events".into())
            .spawn(move || pump(&thread_ipc, reader, &thread_cache, &tx))
            .map_err(|e| Error::Io {
                path: ipc.path.clone(),
                source: e,
            })?;
        Ok(SwayWindows {
            ipc,
            cache,
            rx,
            sub,
        })
    }
}

fn pump(
    ipc: &Ipc,
    mut sub: UnixStream,
    cache: &Mutex<Vec<WindowInfo>>,
    tx: &async_channel::Sender<WindowEvent>,
) {
    loop {
        let (kind, payload) = match ipc::read_message(&mut sub, &ipc.path, None) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("sway event stream ended: {e}");
                return;
            }
        };
        let fresh = match tree(ipc) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("sway tree after an event: {e}");
                continue;
            }
        };
        let mut events = {
            let mut cached = cache.lock().unwrap_or_else(PoisonError::into_inner);
            let events = tree::diff(&cached, &fresh);
            *cached = fresh;
            events
        };
        if kind == WINDOW_EVENT {
            match ipc::parse::<tree::WindowChange>(&payload) {
                Ok(change) => events.extend(change.focused().map(WindowEvent::Focused)),
                Err(e) => tracing::debug!("sway window event: {e}"),
            }
        }
        for e in events {
            if tx.try_send(e).is_err() {
                tracing::debug!("sway window event dropped: no reader keeps up");
            }
        }
    }
}

impl Drop for SwayWindows {
    fn drop(&mut self) {
        let _ = self.sub.shutdown(Shutdown::Both);
    }
}

impl WindowList for SwayWindows {
    // The cache is the event reader's baseline; writing it here would hide changes from its diff.
    fn snapshot(&self) -> Vec<WindowInfo> {
        tree(&self.ipc).unwrap_or_else(|e| {
            tracing::warn!("sway tree: {e}; using the last one");
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        })
    }
    fn has_geometry(&self) -> bool {
        true
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.rx.clone()
    }
}

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "wlroots",
    probe: detect,
    build: |p| Ok(Box::new(SwayWindows::start(ipc(p)?)?)),
};

/// Sway has no pointer query, so the focused output stands in for the one under the pointer.
struct SwayPointer(Ipc);

impl SwayPointer {
    fn focused(&self) -> Result<Option<OutputId>> {
        Ok(tree::focused_output(
            &self.0.json::<Vec<tree::Output>>(GET_OUTPUTS, b"")?,
        ))
    }
}

impl PointerQuery for SwayPointer {
    fn pointer_global(&self) -> Option<Point> {
        None
    }
    fn output_under_pointer(&self) -> Option<OutputId> {
        self.focused_output()
    }
    fn focused_output(&self) -> Option<OutputId> {
        self.focused()
            .inspect_err(|e| tracing::warn!("sway outputs: {e}"))
            .ok()
            .flatten()
    }
}

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "wlroots",
    probe: detect,
    build: |p| {
        let pointer = SwayPointer(ipc(p)?);
        pointer.focused()?;
        Ok(Box::new(pointer))
    },
};

#[derive(serde::Deserialize)]
struct Outcome {
    success: bool,
    error: Option<String>,
}

struct SwayBinds {
    ipc: Ipc,
    config: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl BindHelper for SwayBinds {
    fn syntax(&self) -> BindSyntax {
        BindSyntax::Sway
    }
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String {
        binds::snippet(binds)
    }
    fn existing_binds(&self) -> Result<Vec<(Trigger, String)>> {
        #[derive(serde::Deserialize)]
        struct Config {
            config: String,
        }
        let c: Config = self.ipc.json(GET_CONFIG, b"")?;
        Ok(binds::parse_config(&c.config))
    }
    fn file_name(&self) -> &'static str {
        "binds.conf"
    }
    fn file_text(&self, binds: &[(Trigger, Route)]) -> String {
        binds::file(binds)
    }
    fn include_line(&self, dir: &str) -> String {
        format!("include {dir}/binds.conf")
    }
    fn is_included(&self, dir: &Path) -> Result<bool> {
        let main = self
            .config
            .as_deref()
            .ok_or_else(|| Error::NotFound("sway config (no HOME)".into()))?;
        include_scan::scan(main, self.home.as_deref(), &include_scan::SWAY, dir)
    }
    fn needs_reload(&self) -> bool {
        true
    }
    fn reload(&self) -> Result<()> {
        let outcomes: Vec<Outcome> = self.ipc.json(RUN_COMMAND, b"reload")?;
        match outcomes.into_iter().find(|o| !o.success) {
            Some(o) => Err(Error::Protocol(
                o.error
                    .unwrap_or_else(|| "sway could not reload".to_owned()),
            )),
            None => Ok(()),
        }
    }
}

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "wlroots",
    probe: detect,
    build: |p| {
        let config = include_scan::config_home(&p.env).map(|c| c.join("sway").join("config"));
        Ok(Box::new(SwayBinds {
            ipc: ipc(p)?,
            config,
            home: include_scan::home(&p.env),
        }))
    },
};
