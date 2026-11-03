use super::ipc::Ipc;
use super::json::Bind;
use crate::error::Result;
use crate::include_scan;
use crate::traits::{BindHelper, BindSyntax, RuleSet, Trigger, WindowRules};
use es_protocol::Route;
use std::path::{Path, PathBuf};

pub const LAYER_NAMESPACES: [&str; 8] = [
    "easyshot-overlay",
    "easyshot-countdown",
    "easyshot-toast",
    "easyshot-deck",
    "easyshot-bar",
    "easyshot-rec-highlight",
    "easyshot-scroll",
    "easyshot-pin",
];

const EDITOR_CLASS: &str = r"^dev\\.easyshot\\.editor$";

/// Lua state outlives the daemon, so the rules are created once per compositor session and re-enabled after.
pub fn install_script(float_editor: bool) -> String {
    let layers: Vec<String> = LAYER_NAMESPACES
        .iter()
        .map(|ns| {
            format!(r#"hl.layer_rule({{ name = "{ns}", match = {{ namespace = "^{ns}$" }}, no_anim = true }})"#)
        })
        .collect();
    let mut script = format!(
        "if not EASYSHOT_RULES then EASYSHOT_RULES = {{ {} }} end \
         for _, r in ipairs(EASYSHOT_RULES) do r:set_enabled(true) end",
        layers.join(", ")
    );
    if float_editor {
        script.push_str(&format!(
            r#" if not EASYSHOT_EDITOR_RULE then EASYSHOT_EDITOR_RULE = hl.window_rule({{ name = "easyshot-editor", match = {{ class = "{EDITOR_CLASS}" }}, float = true, center = true, size = {{ 1180, 780 }} }}) end EASYSHOT_EDITOR_RULE:set_enabled(true)"#
        ));
    } else {
        script
            .push_str(" if EASYSHOT_EDITOR_RULE then EASYSHOT_EDITOR_RULE:set_enabled(false) end");
    }
    script
}

pub fn remove_script() -> String {
    "if EASYSHOT_RULES then for _, r in ipairs(EASYSHOT_RULES) do r:set_enabled(false) end end \
     if EASYSHOT_EDITOR_RULE then EASYSHOT_EDITOR_RULE:set_enabled(false) end"
        .to_owned()
}

pub struct HyprRules {
    pub ipc: Ipc,
}

impl WindowRules for HyprRules {
    fn install(&self, rules: &RuleSet) -> Result<()> {
        self.ipc.eval(&install_script(rules.float_editor))
    }
    fn remove(&self) -> Result<()> {
        self.ipc.eval(&remove_script())
    }
}

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
