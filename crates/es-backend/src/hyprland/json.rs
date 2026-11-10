use crate::traits::Trigger;
use es_geom::{Point, Rect};
use es_model::platform::{OutputId, WindowId, WindowInfo};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkspaceRef {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub address: String,
    #[serde(default)]
    pub mapped: bool,
    #[serde(default)]
    pub hidden: bool,
    pub at: [f64; 2],
    pub size: [f64; 2],
    #[serde(default)]
    pub workspace: WorkspaceRef,
    #[serde(default)]
    pub floating: bool,
    #[serde(default)]
    pub fullscreen: serde_json::Value,
    #[serde(default)]
    pub monitor: i64,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub pid: i64,
    #[serde(default, rename = "focusHistoryID")]
    pub focus_history_id: i64,
}

impl Client {
    // Older releases send a bool, newer ones a fullscreen mode number.
    fn is_fullscreen(&self) -> bool {
        self.fullscreen.as_bool().unwrap_or(false) || self.fullscreen.as_i64().unwrap_or(0) > 0
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    #[serde(default)]
    pub transform: i64,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub active_workspace: WorkspaceRef,
    #[serde(default)]
    pub special_workspace: WorkspaceRef,
}

impl Monitor {
    /// `width` and `height` are the mode in pixels, before transform and scale.
    pub fn logical(&self) -> Rect {
        let (w, h) = if self.transform % 2 == 1 {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
        Rect::new(self.x, self.y, self.x + w / scale, self.y + h / scale)
    }

    fn shows(&self, workspace: i64) -> bool {
        !self.disabled
            && (workspace == self.active_workspace.id
                || (self.special_workspace.id != 0 && workspace == self.special_workspace.id))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CursorPos {
    pub x: f64,
    pub y: f64,
}

impl From<CursorPos> for Point {
    fn from(c: CursorPos) -> Point {
        Point::new(c.x, c.y)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Bind {
    #[serde(default)]
    pub modmask: u32,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub dispatcher: String,
    #[serde(default)]
    pub arg: String,
    #[serde(default)]
    pub submap: String,
}

const MODS: &[(u32, &str)] = &[
    (64, "SUPER"),
    (4, "CTRL"),
    (8, "ALT"),
    (1, "SHIFT"),
    (2, "CAPS"),
    (16, "MOD2"),
    (32, "MOD3"),
    (128, "MOD5"),
];

impl Bind {
    pub fn trigger(&self) -> Trigger {
        let mut parts: Vec<&str> = MODS
            .iter()
            .filter(|(bit, _)| self.modmask & bit != 0)
            .map(|(_, name)| *name)
            .collect();
        parts.push(&self.key);
        Trigger(parts.join("+"))
    }

    pub fn action(&self) -> String {
        format!("{} {}", self.dispatcher, self.arg)
            .trim()
            .to_owned()
    }
}

pub fn address(raw: &str) -> String {
    if raw.starts_with("0x") {
        raw.to_owned()
    } else {
        format!("0x{raw}")
    }
}

pub fn window_info(c: &Client, monitors: &[Monitor]) -> WindowInfo {
    let [x, y] = c.at;
    let [w, h] = c.size;
    WindowInfo {
        id: WindowId(address(&c.address)),
        app_id: c.class.clone(),
        title: c.title.clone(),
        pid: u32::try_from(c.pid).ok().filter(|p| *p > 0),
        logical: Some(Rect::new(x, y, x + w, y + h)),
        output: monitors
            .iter()
            .find(|m| m.id == c.monitor)
            .map(|m| OutputId(m.name.clone())),
        workspace: Some(c.workspace.name.clone()),
        floating: c.floating,
        focus_rank: u32::try_from(c.focus_history_id).ok(),
    }
}

/// Windows on a workspace some monitor shows, topmost first: fullscreen, then floating, then by focus.
pub fn visible_windows(clients: &[Client], monitors: &[Monitor]) -> Vec<WindowInfo> {
    let mut shown: Vec<&Client> = clients
        .iter()
        .filter(|c| c.mapped && !c.hidden)
        .filter(|c| {
            monitors
                .iter()
                .any(|m| m.id == c.monitor && m.shows(c.workspace.id))
        })
        .collect();
    shown.sort_by_key(|c| (!c.is_fullscreen(), !c.floating, c.focus_history_id));
    shown.iter().map(|c| window_info(c, monitors)).collect()
}

/// Every mapped window on a numbered workspace or on one a monitor shows, shown windows first.
pub fn all_windows(clients: &[Client], monitors: &[Monitor]) -> Vec<WindowInfo> {
    let mut shown = visible_windows(clients, monitors);
    let seen: Vec<String> = shown.iter().map(|w| w.id.0.clone()).collect();
    let mut hidden: Vec<&Client> = clients
        .iter()
        .filter(|c| c.mapped && !c.hidden)
        .filter(|c| c.workspace.id > 0)
        .filter(|c| !seen.contains(&address(&c.address)))
        .collect();
    hidden.sort_by_key(|c| (c.workspace.id, c.focus_history_id));
    shown.extend(hidden.iter().map(|c| WindowInfo {
        logical: None,
        ..window_info(c, monitors)
    }));
    shown
}

pub fn focus_command(addr: &str, lua: bool) -> String {
    let addr = address(addr);
    if lua {
        format!("dispatch hl.dsp.focus({{ window = 'address:{addr}' }})")
    } else {
        format!("dispatch focuswindow address:{addr}")
    }
}

pub fn output_at(monitors: &[Monitor], p: Point) -> Option<OutputId> {
    monitors
        .iter()
        .filter(|m| !m.disabled)
        .find(|m| {
            let r = m.logical();
            p.x >= r.x0 && p.x < r.x1 && p.y >= r.y0 && p.y < r.y1
        })
        .map(|m| OutputId(m.name.clone()))
}

pub fn focused_monitor(monitors: &[Monitor]) -> Option<OutputId> {
    monitors
        .iter()
        .find(|m| m.focused && !m.disabled)
        .map(|m| OutputId(m.name.clone()))
}

/// Address to foreign-toplevel identifier, by class and title; a pair seen twice on either side stays unmapped.
pub fn join_toplevels(clients: &[Client], toplevels: &[WindowInfo]) -> HashMap<String, WindowId> {
    let mut counts: HashMap<(&str, &str), (usize, Option<&WindowId>)> = HashMap::new();
    for t in toplevels {
        let e = counts
            .entry((t.app_id.as_str(), t.title.as_str()))
            .or_default();
        e.0 += 1;
        e.1 = Some(&t.id);
    }
    let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
    for c in clients.iter().filter(|c| c.mapped) {
        *seen
            .entry((c.class.as_str(), c.title.as_str()))
            .or_default() += 1;
    }
    clients
        .iter()
        .filter(|c| c.mapped)
        .filter_map(|c| {
            let key = (c.class.as_str(), c.title.as_str());
            match (counts.get(&key), seen.get(&key)) {
                (Some((1, Some(id))), Some(1)) => Some((address(&c.address), (*id).clone())),
                _ => None,
            }
        })
        .collect()
}
