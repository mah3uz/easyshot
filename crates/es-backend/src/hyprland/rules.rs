use super::ipc::Ipc;
use super::json::Bind;
use crate::error::Result;
use crate::include_scan;
use crate::traits::{BindHelper, BindSyntax, Trigger};
use es_protocol::Route;
use std::path::{Path, PathBuf};

fn command(route: &Route) -> String {
    format!("easyshot {}", route.command_args().join(" "))
}

fn split(trigger: &Trigger) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = trigger.0.split('+').map(str::trim).collect();
    let key = parts.pop().unwrap_or_default();
    (parts, key)
}

fn lua_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn lua_snippet(binds: &[(Trigger, Route)]) -> String {
    binds
        .iter()
        .map(|(t, r)| {
            let (mut parts, key) = split(t);
            parts.push(key);
            format!(
                "hl.bind(\"{}\", hl.dsp.exec_cmd(\"{}\"))\n",
                lua_str(&parts.join(" + ")),
                lua_str(&command(r))
            )
        })
        .collect()
}

pub fn hyprlang_snippet(binds: &[(Trigger, Route)]) -> String {
    binds
        .iter()
        .map(|(t, r)| {
            let (mods, key) = split(t);
            format!("bind = {}, {key}, exec, {}\n", mods.join(" "), command(r))
        })
        .collect()
}

pub fn parse_binds(json: &[u8]) -> Result<Vec<(Trigger, String)>> {
    let binds: Vec<Bind> = serde_json::from_slice(json)
        .map_err(|e| crate::Error::Protocol(format!("hyprland binds: {e}")))?;
    Ok(binds
        .iter()
        .filter(|b| b.submap.is_empty())
        .map(|b| (b.trigger(), b.action()))
        .collect())
}

pub struct HyprBinds {
    pub ipc: Ipc,
    pub syntax: BindSyntax,
    /// The main config file, `hyprland.lua` or `hyprland.conf`.
    pub config: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

const HEADER: &str =
    "easyshot rewrites this file when you change its shortcuts. Edit them in easyshot.";

fn lua_file(binds: &[(Trigger, Route)]) -> String {
    format!("-- {HEADER}\n{}", lua_snippet(binds))
}

fn hyprlang_file(binds: &[(Trigger, Route)]) -> String {
    format!("# {HEADER}\n{}", hyprlang_snippet(binds))
}

impl BindHelper for HyprBinds {
    fn syntax(&self) -> BindSyntax {
        self.syntax
    }
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String {
        match self.syntax {
            BindSyntax::Hyprlang => hyprlang_snippet(binds),
            _ => lua_snippet(binds),
        }
    }
    fn existing_binds(&self) -> Result<Vec<(Trigger, String)>> {
        parse_binds(&self.ipc.request("j/binds")?)
    }
    fn file_name(&self) -> &'static str {
        match self.syntax {
            BindSyntax::Hyprlang => "binds.conf",
            _ => "binds.lua",
        }
    }
    fn file_text(&self, binds: &[(Trigger, Route)]) -> String {
        match self.syntax {
            BindSyntax::Hyprlang => hyprlang_file(binds),
            _ => lua_file(binds),
        }
    }
    fn include_line(&self, dir: &str) -> String {
        match self.syntax {
            BindSyntax::Hyprlang => format!("source = {dir}/binds.conf"),
            _ => format!("pcall(require, \"{dir}/binds\")"),
        }
    }
    fn is_included(&self, dir: &Path) -> Result<bool> {
        let main = self
            .config
            .as_deref()
            .ok_or_else(|| crate::Error::NotFound("hyprland config (no HOME)".into()))?;
        let rule = match self.syntax {
            BindSyntax::Hyprlang => &include_scan::HYPRLANG,
            _ => &include_scan::LUA,
        };
        include_scan::scan(main, self.home.as_deref(), rule, dir)
    }
}
