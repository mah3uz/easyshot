mod ipc;
mod json;
mod rules;
mod windows;

pub use ipc::{EVENT_SOCKET, Ipc, REQUEST_SOCKET, REQUEST_TIMEOUT};
pub use json::{
    Client, CursorPos, Monitor, focused_monitor, join_toplevels, output_at, visible_windows,
};
pub use rules::{HyprBinds, hyprlang_snippet, lua_snippet, parse_binds};
pub use windows::{HyprEvent, HyprWindows, Toplevels, parse_event};

use crate::error::BackendError;
use crate::probe::{Env, Probe, Provider};
use crate::traits::{BindHelper, BindSyntax, PointerQuery, WindowList};
use es_geom::Point;
use es_model::platform::{CursorSource, OutputId};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const PRIORITY: u8 = 30;

pub const SIGNATURE_VAR: &str = "HYPRLAND_INSTANCE_SIGNATURE";

pub fn socket_dir(env: &Env) -> Option<PathBuf> {
    let signature = env.var(SIGNATURE_VAR)?;
    let runtime = env.var("XDG_RUNTIME_DIR")?;
    let hyprland = env
        .current_desktop()?
        .split(':')
        .any(|d| d.eq_ignore_ascii_case("hyprland"));
    let plain = !signature.contains('/') && signature != "..";
    (hyprland && plain).then(|| Path::new(runtime).join("hypr").join(signature))
}

fn detected(p: &Probe) -> Option<u8> {
    socket_dir(&p.env).map(|_| PRIORITY)
}

fn ipc(p: &Probe) -> Result<Ipc, BackendError> {
    let ipc = socket_dir(&p.env)
        .map(Ipc::new)
        .ok_or_else(|| BackendError::Unavailable("not a Hyprland session".into()))?;
    if !ipc.has_socket() {
        return Err(BackendError::Unavailable(format!(
            "no Hyprland socket in {}",
            ipc.dir().display()
        )));
    }
    Ok(ipc)
}

fn hypr_dir(env: &Env) -> Option<PathBuf> {
    crate::include_scan::config_home(env).map(|c| c.join("hypr"))
}

fn config_is_lua(env: &Env) -> bool {
    hypr_dir(env).is_some_and(|d| d.join("hyprland.lua").exists())
}

pub struct HyprPointer {
    pub ipc: Ipc,
}

impl HyprPointer {
    fn cursor(&self) -> Option<Point> {
        self.ipc
            .json::<CursorPos>("cursorpos")
            .inspect_err(|e| tracing::debug!("hyprland: cursorpos: {e}"))
            .ok()
            .map(Point::from)
    }

    fn monitors(&self) -> Option<Vec<Monitor>> {
        self.ipc
            .json("monitors")
            .inspect_err(|e| tracing::debug!("hyprland: monitors: {e}"))
            .ok()
    }
}

impl PointerQuery for HyprPointer {
    fn pointer_global(&self) -> Option<Point> {
        self.cursor()
    }
    fn output_under_pointer(&self) -> Option<OutputId> {
        let at = self.cursor()?;
        output_at(&self.monitors()?, at)
    }
    fn focused_output(&self) -> Option<OutputId> {
        focused_monitor(&self.monitors()?)
    }
}

impl CursorSource for HyprPointer {
    fn sample(&self) -> Option<(Point, Instant)> {
        self.cursor().map(|p| (p, Instant::now()))
    }
}

pub const WINDOWS: Provider<dyn WindowList> = Provider {
    name: "hyprland",
    probe: detected,
    build: |p| {
        let toplevels = p.wayland.clone().map(|conn| {
            let source: Toplevels = Arc::new(move || conn.windows());
            source
        });
        Ok(Box::new(HyprWindows::start(ipc(p)?, toplevels)?))
    },
};

pub const POINTER: Provider<dyn PointerQuery> = Provider {
    name: "hyprland",
    probe: detected,
    build: |p| Ok(Box::new(HyprPointer { ipc: ipc(p)? })),
};

pub const BINDS: Provider<dyn BindHelper> = Provider {
    name: "hyprland",
    probe: detected,
    build: |p| {
        let syntax = if config_is_lua(&p.env) {
            BindSyntax::HyprLua
        } else {
            BindSyntax::Hyprlang
        };
        let file = match syntax {
            BindSyntax::Hyprlang => "hyprland.conf",
            _ => "hyprland.lua",
        };
        Ok(Box::new(HyprBinds {
            ipc: ipc(p)?,
            syntax,
            config: hypr_dir(&p.env).map(|d| d.join(file)),
            home: crate::include_scan::home(&p.env),
        }))
    },
};

pub const CURSOR: Provider<dyn CursorSource> = Provider {
    name: "hyprland",
    probe: |p| detected(p).map(|_| 10),
    build: |p| Ok(Box::new(HyprPointer { ipc: ipc(p)? })),
};
