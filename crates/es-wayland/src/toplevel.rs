use crate::worker::State;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    self, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    self, ExtForeignToplevelListV1,
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
            }
            ext_foreign_toplevel_handle_v1::Event::Closed => {
                state.toplevels.remove(i).handle.destroy();
            }
            _ => {}
        }
    }
}
