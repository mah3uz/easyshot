use super::wire::{Event, Output, Window, Workspace};
use es_geom::Rect;
use es_model::platform::{OutputId, WindowEvent, WindowId, WindowInfo};
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub(crate) struct State {
    pub(crate) windows: BTreeMap<u64, Window>,
    pub(crate) workspaces: BTreeMap<u64, Workspace>,
    pub(crate) outputs: HashMap<String, Output>,
}

/// Global logical rect of the visible part of `w`; tiles scrolled out of view have none.
pub(crate) fn visible_rect(w: &Window, ws: &Workspace, output: &Output) -> Option<Rect> {
    if !ws.is_active {
        return None;
    }
    let o = output.logical?;
    let (tile_x, tile_y) = w.layout.tile_pos_in_workspace_view?;
    let (off_x, off_y) = w.layout.window_offset_in_tile;
    let (width, height) = w.layout.window_size;
    let x = f64::from(o.x) + tile_x + off_x;
    let y = f64::from(o.y) + tile_y + off_y;
    let window = Rect::new(x, y, x + f64::from(width), y + f64::from(height));
    let screen = Rect::new(
        f64::from(o.x),
        f64::from(o.y),
        f64::from(o.x) + f64::from(o.width),
        f64::from(o.y) + f64::from(o.height),
    );
    let seen = window.intersect(screen);
    (seen.width() > 0.0 && seen.height() > 0.0).then_some(seen)
}

impl State {
    /// Niri has no stacking order: the focused window ranks first, then by last focus.
    pub(crate) fn infos(&self) -> Vec<WindowInfo> {
        let mut order: Vec<&Window> = self.windows.values().collect();
        order.sort_by_key(|w| {
            std::cmp::Reverse((w.is_focused, w.focus_timestamp.map(|t| (t.secs, t.nanos))))
        });
        order
            .iter()
            .enumerate()
            .map(|(rank, w)| self.info(w, rank))
            .collect()
    }

    fn info(&self, w: &Window, rank: usize) -> WindowInfo {
        let ws = w.workspace_id.and_then(|id| self.workspaces.get(&id));
        let output = ws.and_then(|ws| ws.output.as_ref());
        let logical = ws
            .zip(output.and_then(|o| self.outputs.get(o)))
            .and_then(|(ws, o)| visible_rect(w, ws, o));
        WindowInfo {
            id: WindowId(w.id.to_string()),
            app_id: w.app_id.clone().unwrap_or_default(),
            title: w.title.clone().unwrap_or_default(),
            pid: w.pid.and_then(|p| u32::try_from(p).ok()),
            logical,
            output: output.cloned().map(OutputId),
            workspace: ws.map(|ws| ws.name.clone().unwrap_or_else(|| ws.idx.to_string())),
            floating: w.is_floating,
            focus_rank: Some(u32::try_from(rank).unwrap_or(u32::MAX)),
        }
    }

    /// Returns the newly focused window, if the event moved focus.
    pub(crate) fn apply(&mut self, event: Event) -> Option<WindowId> {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspaces = workspaces.into_iter().map(|ws| (ws.id, ws)).collect();
            }
            Event::WorkspaceActivated { id, focused } => {
                let output = self.workspaces.get(&id).and_then(|ws| ws.output.clone());
                for ws in self.workspaces.values_mut() {
                    if ws.output == output {
                        ws.is_active = ws.id == id;
                    }
                    if focused {
                        ws.is_focused = ws.id == id;
                    }
                }
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();
            }
            Event::WindowOpenedOrChanged { window } => {
                let was_focused = self.windows.get(&window.id).is_some_and(|w| w.is_focused);
                let focused = (window.is_focused && !was_focused).then_some(window.id);
                if focused.is_some() {
                    self.windows.values_mut().for_each(|w| w.is_focused = false);
                }
                self.windows.insert(window.id, window);
                return focused.map(|id| WindowId(id.to_string()));
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
            }
            Event::WindowFocusChanged { id } => {
                self.windows
                    .values_mut()
                    .for_each(|w| w.is_focused = Some(w.id) == id);
                return id.map(|id| WindowId(id.to_string()));
            }
            Event::WindowFocusTimestampChanged {
                id,
                focus_timestamp,
            } => {
                if let Some(w) = self.windows.get_mut(&id) {
                    w.focus_timestamp = focus_timestamp;
                }
            }
            Event::WindowLayoutsChanged { changes } => {
                for (id, layout) in changes {
                    if let Some(w) = self.windows.get_mut(&id) {
                        w.layout = layout;
                    }
                }
            }
        }
        None
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
