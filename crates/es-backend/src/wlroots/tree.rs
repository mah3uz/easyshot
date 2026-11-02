use es_geom::Rect;
use es_model::platform::{OutputId, WindowEvent, WindowId, WindowInfo};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Default, Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize)]
struct Props {
    class: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Node {
    id: u64,
    #[serde(rename = "type")]
    kind: String,
    name: Option<String>,
    app_id: Option<String>,
    pid: Option<u32>,
    #[serde(default)]
    rect: Area,
    #[serde(default)]
    window_rect: Area,
    visible: Option<bool>,
    window_properties: Option<Props>,
    #[serde(default)]
    focus: Vec<u64>,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    floating_nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Output {
    name: String,
    #[serde(default)]
    focused: bool,
}

pub(crate) fn focused_output(outputs: &[Output]) -> Option<OutputId> {
    outputs
        .iter()
        .find(|o| o.focused)
        .map(|o| OutputId(o.name.clone()))
}

#[derive(Clone, Default)]
struct Place {
    output: Option<String>,
    workspace: Option<String>,
}

impl Node {
    fn is_view(&self) -> bool {
        matches!(self.kind.as_str(), "con" | "floating_con")
            && self.nodes.is_empty()
            && self.floating_nodes.is_empty()
            && (self.pid.is_some() || self.app_id.is_some() || self.window_properties.is_some())
    }

    /// Rects are already logical: sway lays out in scaled coordinates.
    fn content(&self) -> Option<Rect> {
        let (r, w) = (self.rect, self.window_rect);
        if self.visible == Some(false) || w.width <= 0 || w.height <= 0 {
            return None;
        }
        let (x, y) = (f64::from(r.x + w.x), f64::from(r.y + w.y));
        Some(Rect::new(
            x,
            y,
            x + f64::from(w.width),
            y + f64::from(w.height),
        ))
    }

    fn info(&self, place: &Place, rank: u32) -> WindowInfo {
        let app_id = self
            .app_id
            .clone()
            .or_else(|| {
                self.window_properties
                    .as_ref()
                    .and_then(|p| p.class.clone())
            })
            .unwrap_or_default();
        WindowInfo {
            id: WindowId(self.id.to_string()),
            app_id,
            title: self.name.clone().unwrap_or_default(),
            pid: self.pid,
            logical: self.content(),
            output: place.output.clone().map(OutputId),
            workspace: place.workspace.clone(),
            floating: self.kind == "floating_con",
            focus_rank: Some(rank),
        }
    }

    /// Children most recently focused first, then the rest in tree order.
    fn children_by_focus(&self) -> Vec<&Node> {
        let all = || self.nodes.iter().chain(&self.floating_nodes);
        let mut out: Vec<&Node> = self
            .focus
            .iter()
            .filter_map(|id| all().find(|c| c.id == *id))
            .collect();
        out.extend(all().filter(|c| !self.focus.contains(&c.id)));
        out
    }
}

/// Every view, ranked by focus: rank 0 was focused last.
pub(crate) fn windows(root: &Node) -> Vec<WindowInfo> {
    let mut out = Vec::new();
    walk(root, Place::default(), &mut out);
    out
}

fn walk(n: &Node, mut place: Place, out: &mut Vec<WindowInfo>) {
    match n.kind.as_str() {
        "output" => place.output = n.name.clone().filter(|name| name != "__i3"),
        "workspace" => place.workspace = n.name.clone(),
        _ => {}
    }
    if n.is_view() {
        let rank = u32::try_from(out.len()).unwrap_or(u32::MAX);
        out.push(n.info(&place, rank));
        return;
    }
    for c in n.children_by_focus() {
        walk(c, place.clone(), out);
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct WindowChange {
    change: String,
    container: ContainerId,
}

#[derive(Debug, Deserialize)]
struct ContainerId {
    id: u64,
}

impl WindowChange {
    pub(crate) fn focused(&self) -> Option<WindowId> {
        (self.change == "focus").then(|| WindowId(self.container.id.to_string()))
    }
}

fn same_place(a: &WindowInfo, b: &WindowInfo) -> bool {
    WindowInfo {
        focus_rank: None,
        ..a.clone()
    } == WindowInfo {
        focus_rank: None,
        ..b.clone()
    }
}

/// Focus rank changes alone are not reported; `Focused` carries them.
pub(crate) fn diff(old: &[WindowInfo], new: &[WindowInfo]) -> Vec<WindowEvent> {
    let before: BTreeMap<&WindowId, &WindowInfo> = old.iter().map(|w| (&w.id, w)).collect();
    let after: BTreeMap<&WindowId, &WindowInfo> = new.iter().map(|w| (&w.id, w)).collect();
    let mut events: Vec<WindowEvent> = old
        .iter()
        .filter(|w| !after.contains_key(&w.id))
        .map(|w| WindowEvent::Closed(w.id.clone()))
        .collect();
    for w in new {
        match before.get(&w.id) {
            None => events.push(WindowEvent::Opened(w.clone())),
            Some(prev) if !same_place(prev, w) => events.push(WindowEvent::Changed(w.clone())),
            Some(_) => {}
        }
    }
    events
}
