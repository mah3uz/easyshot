use crate::worker::State;
use es_model::platform::{WindowEvent, WindowId, WindowInfo};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toplevel {
    pub identifier: String,
    pub app_id: String,
    pub title: String,
}

pub(crate) struct ToplevelState {
    pub handle: ExtForeignToplevelHandleV1,
    pub info: Toplevel,
    pub done: bool,
    sent: Option<Toplevel>,
}

pub(crate) struct WlrToplevel {
    handle: ZwlrForeignToplevelHandleV1,
    app_id: String,
    title: String,
    pending_activated: bool,
    activated: bool,
    paired: Option<String>,
}

fn window_info(t: &Toplevel, focus_rank: Option<u32>) -> WindowInfo {
    WindowInfo {
        id: WindowId(t.identifier.clone()),
        app_id: t.app_id.clone(),
        title: t.title.clone(),
        pid: None,
        logical: None,
        output: None,
        workspace: None,
        floating: false,
        focus_rank,
    }
}

impl State {
    pub(crate) fn toplevel_list(&self) -> Vec<Toplevel> {
        self.toplevels
            .iter()
            .filter(|t| t.done && !t.info.identifier.is_empty())
            .map(|t| t.info.clone())
            .collect()
    }

    pub(crate) fn toplevel_handle(&self, identifier: &str) -> Option<&ExtForeignToplevelHandleV1> {
        self.toplevels
            .iter()
            .find(|t| t.done && t.info.identifier == identifier)
            .map(|t| &t.handle)
    }

    fn focus_rank(&self, identifier: &str) -> Option<u32> {
        self.focus_order
            .iter()
            .position(|id| id == identifier)
            .and_then(|i| u32::try_from(i).ok())
    }

    fn publish_windows(&self) {
        let list = self
            .toplevels
            .iter()
            .filter_map(|t| t.sent.as_ref())
            .map(|t| window_info(t, self.focus_rank(&t.identifier)))
            .collect();
        *self
            .window_snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = list;
    }

    fn emit(&self, event: WindowEvent) {
        self.publish_windows();
        // A consumer that falls behind loses the oldest events and resyncs from the snapshot.
        let _ = self.window_tx.force_send(event);
    }

    fn toplevel_done(&mut self, i: usize) {
        let t = &mut self.toplevels[i];
        if t.info.identifier.is_empty() || t.sent.as_ref() == Some(&t.info) {
            return;
        }
        let current = t.info.clone();
        let opened = t.sent.replace(current.clone()).is_none();
        let info = window_info(&current, self.focus_rank(&current.identifier));
        self.emit(if opened {
            WindowEvent::Opened(info)
        } else {
            WindowEvent::Changed(info)
        });
        if opened {
            for w in 0..self.wlr_toplevels.len() {
                self.pair(w);
            }
        }
    }

    fn toplevel_closed(&mut self, i: usize) {
        let t = self.toplevels.remove(i);
        t.handle.destroy();
        let Some(sent) = t.sent else { return };
        self.focus_order.retain(|id| *id != sent.identifier);
        for w in &mut self.wlr_toplevels {
            if w.paired.as_ref() == Some(&sent.identifier) {
                w.paired = None;
            }
        }
        self.emit(WindowEvent::Closed(WindowId(sent.identifier)));
    }

    // The two protocols share no identifier, so a wlr handle is matched to the first unclaimed ext window with the same app id and title.
    fn pair(&mut self, w: usize) {
        let wlr = &self.wlr_toplevels[w];
        if wlr.paired.is_some() {
            return;
        }
        let claimed = |id: &str| {
            self.wlr_toplevels
                .iter()
                .any(|o| o.paired.as_deref() == Some(id))
        };
        let Some(id) = self
            .toplevels
            .iter()
            .filter_map(|t| t.sent.as_ref())
            .find(|t| t.app_id == wlr.app_id && t.title == wlr.title && !claimed(&t.identifier))
            .map(|t| t.identifier.clone())
        else {
            return;
        };
        let activated = wlr.activated;
        self.wlr_toplevels[w].paired = Some(id.clone());
        if activated {
            self.focus(id);
        }
    }

    fn focus(&mut self, id: String) {
        if self.focus_order.first() == Some(&id) {
            return;
        }
        self.focus_order.retain(|o| *o != id);
        self.focus_order.insert(0, id.clone());
        self.emit(WindowEvent::Focused(WindowId(id)));
    }

    fn wlr_done(&mut self, w: usize) {
        let t = &mut self.wlr_toplevels[w];
        let became_active = t.pending_activated && !t.activated;
        t.activated = t.pending_activated;
        if let Some(id) = t.paired.clone() {
            if became_active {
                self.focus(id);
            }
        } else {
            self.pair(w);
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = event {
            state.toplevels.push(ToplevelState {
                handle: toplevel,
                info: Toplevel {
                    identifier: String::new(),
                    app_id: String::new(),
                    title: String::new(),
                },
                done: false,
                sent: None,
            });
        }
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        let Some(i) = state.toplevels.iter().position(|t| t.handle.id() == id) else {
            return;
        };
        let t = &mut state.toplevels[i];
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                t.info.identifier = identifier
            }
            ext_foreign_toplevel_handle_v1::Event::AppId { app_id } => t.info.app_id = app_id,
            ext_foreign_toplevel_handle_v1::Event::Title { title } => t.info.title = title,
            ext_foreign_toplevel_handle_v1::Event::Done => {
                t.done = true;
                tracing::debug!(app_id = %t.info.app_id, title = %t.info.title, "toplevel");
                state.toplevel_done(i);
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => state.toplevel_closed(i),
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        manager: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
                state.wlr_toplevels.push(WlrToplevel {
                    handle: toplevel,
                    app_id: String::new(),
                    title: String::new(),
                    pending_activated: false,
                    activated: false,
                    paired: None,
                })
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                manager.stop();
                state.wlr_toplevel_manager = None;
            }
            _ => {}
        }
    }

    event_created_child!(State, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id();
        let Some(w) = state.wlr_toplevels.iter().position(|t| t.handle.id() == id) else {
            return;
        };
        let t = &mut state.wlr_toplevels[w];
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => t.app_id = app_id,
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => t.title = title,
            zwlr_foreign_toplevel_handle_v1::Event::State { state: flags } => {
                t.pending_activated = is_activated(&flags)
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => state.wlr_done(w),
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                state.wlr_toplevels.remove(w).handle.destroy();
            }
            _ => {}
        }
    }
}

fn is_activated(flags: &[u8]) -> bool {
    let activated = zwlr_foreign_toplevel_handle_v1::State::Activated as u32;
    flags
        .as_chunks::<4>()
        .0
        .iter()
        .any(|w| u32::from_ne_bytes(*w) == activated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_activated_flag_is_found_among_the_state_words() {
        let words = |w: &[u32]| w.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>();
        assert!(is_activated(&words(&[0, 2])));
        assert!(!is_activated(&words(&[0, 1, 3])));
        assert!(!is_activated(&[2, 0]));
    }
}
