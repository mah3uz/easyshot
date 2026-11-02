use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) enum Request {
    Outputs,
    Workspaces,
    Windows,
    FocusedOutput,
    EventStream,
}

pub(crate) type Reply = std::result::Result<Response, String>;

#[derive(Debug, Clone, Deserialize)]
pub(crate) enum Response {
    Handled,
    Outputs(HashMap<String, Output>),
    Workspaces(Vec<Workspace>),
    Windows(Vec<Window>),
    FocusedOutput(Option<Output>),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct Output {
    pub(crate) name: String,
    pub(crate) logical: Option<LogicalOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub(crate) struct LogicalOutput {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Workspace {
    pub(crate) id: u64,
    pub(crate) idx: u8,
    pub(crate) name: Option<String>,
    pub(crate) output: Option<String>,
    pub(crate) is_active: bool,
    pub(crate) is_focused: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct Window {
    pub(crate) id: u64,
    pub(crate) title: Option<String>,
    pub(crate) app_id: Option<String>,
    pub(crate) pid: Option<i32>,
    pub(crate) workspace_id: Option<u64>,
    pub(crate) is_focused: bool,
    pub(crate) is_floating: bool,
    pub(crate) layout: WindowLayout,
    pub(crate) focus_timestamp: Option<Timestamp>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Timestamp {
    pub(crate) secs: u64,
    pub(crate) nanos: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub(crate) struct WindowLayout {
    pub(crate) window_size: (i32, i32),
    pub(crate) tile_pos_in_workspace_view: Option<(f64, f64)>,
    pub(crate) window_offset_in_tile: (f64, f64),
}

/// Events easyshot does not track fail to parse and are skipped by the reader.
#[derive(Debug, Clone, Deserialize)]
pub(crate) enum Event {
    WorkspacesChanged {
        workspaces: Vec<Workspace>,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WindowsChanged {
        windows: Vec<Window>,
    },
    WindowOpenedOrChanged {
        window: Window,
    },
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowFocusTimestampChanged {
        id: u64,
        focus_timestamp: Option<Timestamp>,
    },
    WindowLayoutsChanged {
        changes: Vec<(u64, WindowLayout)>,
    },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn reply(json: &str) -> Response {
        serde_json::from_str::<Reply>(json).unwrap().unwrap()
    }

    const OUTPUT: &str = r#"{
        "name": "eDP-1", "make": "BOE", "model": "0x0BCA", "serial": null,
        "physical_size": [290, 190],
        "modes": [{"width": 2256, "height": 1504, "refresh_rate": 59999, "is_preferred": true}],
        "current_mode": 0, "is_custom_mode": false, "vrr_supported": false, "vrr_enabled": false,
        "logical": {"x": -1504, "y": 10, "width": 1003, "height": 1504, "scale": 1.5, "transform": "90"}
    }"#;

    const WINDOW: &str = r#"{
        "id": 42, "title": "notes.md", "app_id": "org.gnome.TextEditor", "pid": 2817,
        "workspace_id": 3, "is_focused": true, "is_floating": false, "is_urgent": false,
        "layout": {"pos_in_scrolling_layout": [2, 1], "tile_size": [960.0, 1080.0],
                   "window_size": [956, 1076], "tile_pos_in_workspace_view": [-120.5, 0.0],
                   "window_offset_in_tile": [2.0, 2.0]},
        "focus_timestamp": {"secs": 1730000000, "nanos": 250000000}
    }"#;

    #[test]
    fn requests_are_bare_variant_names() {
        let sent: Vec<String> = [
            Request::Outputs,
            Request::Workspaces,
            Request::Windows,
            Request::FocusedOutput,
            Request::EventStream,
        ]
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();
        assert_eq!(
            sent,
            [
                r#""Outputs""#,
                r#""Workspaces""#,
                r#""Windows""#,
                r#""FocusedOutput""#,
                r#""EventStream""#
            ]
        );
    }

    #[test]
    fn replies_are_ok_or_err() {
        assert!(matches!(reply(r#"{"Ok":"Handled"}"#), Response::Handled));
        let err: Reply = serde_json::from_str(r#"{"Err":"error parsing request"}"#).unwrap();
        assert_eq!(err.err().as_deref(), Some("error parsing request"));
        assert!(serde_json::from_str::<Reply>(r#"{"Ok":{"Version":"26.04"}}"#).is_err());
    }

    #[test]
    fn outputs_keep_name_and_logical_rect() {
        let json = format!(
            r#"{{"Ok":{{"Outputs":{{"eDP-1":{OUTPUT},"DP-2":{}}}}}}}"#,
            r#"{"name": "DP-2", "make": "Dell", "model": "U2720Q", "serial": "ABC",
                "physical_size": null, "modes": [], "current_mode": null,
                "is_custom_mode": false, "vrr_supported": true, "vrr_enabled": false,
                "logical": null}"#
        );
        let Response::Outputs(outputs) = reply(&json) else {
            panic!("not Outputs");
        };
        let edp = &outputs["eDP-1"];
        assert_eq!(edp.name, "eDP-1");
        assert_eq!(
            edp.logical,
            Some(LogicalOutput {
                x: -1504,
                y: 10,
                width: 1003,
                height: 1504
            })
        );
        assert_eq!(outputs["DP-2"].logical, None, "a disabled output");

        let Response::FocusedOutput(focused) =
            reply(&format!(r#"{{"Ok":{{"FocusedOutput":{OUTPUT}}}}}"#))
        else {
            panic!("not FocusedOutput");
        };
        assert_eq!(focused.map(|o| o.name).as_deref(), Some("eDP-1"));
        assert!(matches!(
            reply(r#"{"Ok":{"FocusedOutput":null}}"#),
            Response::FocusedOutput(None)
        ));
    }

    #[test]
    fn windows_and_workspaces_parse_with_nulls_and_unknown_fields() {
        let json = format!(
            r#"{{"Ok":{{"Windows":[{WINDOW},{}]}}}}"#,
            r#"{"id": 7, "title": null, "app_id": null, "pid": null, "workspace_id": null,
                "is_focused": false, "is_floating": true, "is_urgent": true,
                "layout": {"pos_in_scrolling_layout": null, "tile_size": [640.0, 480.0],
                           "window_size": [640, 480], "tile_pos_in_workspace_view": null,
                           "window_offset_in_tile": [0.0, 0.0], "added_later": 1},
                "focus_timestamp": null, "added_later": {"x": true}}"#
        );
        let Response::Windows(windows) = reply(&json) else {
            panic!("not Windows");
        };
        let w = &windows[0];
        assert_eq!(
            (w.id, w.pid, w.workspace_id, w.is_focused, w.is_floating),
            (42, Some(2817), Some(3), true, false)
        );
        assert_eq!(w.title.as_deref(), Some("notes.md"));
        assert_eq!(w.app_id.as_deref(), Some("org.gnome.TextEditor"));
        assert_eq!(
            w.layout,
            WindowLayout {
                window_size: (956, 1076),
                tile_pos_in_workspace_view: Some((-120.5, 0.0)),
                window_offset_in_tile: (2.0, 2.0),
            }
        );
        assert_eq!(
            w.focus_timestamp,
            Some(Timestamp {
                secs: 1_730_000_000,
                nanos: 250_000_000
            })
        );
        let bare = &windows[1];
        assert_eq!(
            (bare.title.as_ref(), bare.pid, bare.workspace_id),
            (None, None, None)
        );
        assert_eq!(bare.layout.tile_pos_in_workspace_view, None);
        assert_eq!(bare.focus_timestamp, None);

        let Response::Workspaces(workspaces) = reply(
            r#"{"Ok":{"Workspaces":[
                {"id": 3, "idx": 2, "name": "web", "output": "eDP-1", "is_urgent": false,
                 "is_active": true, "is_focused": false, "active_window_id": 42},
                {"id": 4, "idx": 1, "name": null, "output": null, "is_urgent": false,
                 "is_active": false, "is_focused": false, "active_window_id": null}]}}"#,
        ) else {
            panic!("not Workspaces");
        };
        assert_eq!(
            workspaces,
            [
                Workspace {
                    id: 3,
                    idx: 2,
                    name: Some("web".into()),
                    output: Some("eDP-1".into()),
                    is_active: true,
                    is_focused: false,
                },
                Workspace {
                    id: 4,
                    idx: 1,
                    name: None,
                    output: None,
                    is_active: false,
                    is_focused: false,
                },
            ]
        );
    }

    #[test]
    fn tracked_events_parse_and_others_are_rejected() {
        let event = |json: &str| serde_json::from_str::<Event>(json);
        assert!(matches!(
            event(r#"{"WorkspacesChanged":{"workspaces":[]}}"#).unwrap(),
            Event::WorkspacesChanged { workspaces } if workspaces.is_empty()
        ));
        assert!(matches!(
            event(r#"{"WorkspaceActivated":{"id":5,"focused":true}}"#).unwrap(),
            Event::WorkspaceActivated {
                id: 5,
                focused: true
            }
        ));
        assert!(matches!(
            event(r#"{"WindowsChanged":{"windows":[]}}"#).unwrap(),
            Event::WindowsChanged { windows } if windows.is_empty()
        ));
        assert!(matches!(
            event(&format!(r#"{{"WindowOpenedOrChanged":{{"window":{WINDOW}}}}}"#)).unwrap(),
            Event::WindowOpenedOrChanged { window } if window.id == 42
        ));
        assert!(matches!(
            event(r#"{"WindowClosed":{"id":42}}"#).unwrap(),
            Event::WindowClosed { id: 42 }
        ));
        assert!(matches!(
            event(r#"{"WindowFocusChanged":{"id":null}}"#).unwrap(),
            Event::WindowFocusChanged { id: None }
        ));
        assert!(matches!(
            event(r#"{"WindowFocusTimestampChanged":{"id":42,"focus_timestamp":{"secs":9,"nanos":1}}}"#)
                .unwrap(),
            Event::WindowFocusTimestampChanged {
                id: 42,
                focus_timestamp: Some(Timestamp { secs: 9, nanos: 1 })
            }
        ));
        let Event::WindowLayoutsChanged { changes } = event(
            r#"{"WindowLayoutsChanged":{"changes":[[42,{"pos_in_scrolling_layout":[1,1],
                "tile_size":[100.0,50.0],"window_size":[96,46],
                "tile_pos_in_workspace_view":[8.0,4.0],"window_offset_in_tile":[2.0,2.0]}]]}}"#,
        )
        .unwrap() else {
            panic!("not WindowLayoutsChanged");
        };
        assert_eq!(changes[0].0, 42);
        assert_eq!(changes[0].1.tile_pos_in_workspace_view, Some((8.0, 4.0)));

        for untracked in [
            r#"{"WorkspaceActiveWindowChanged":{"workspace_id":3,"active_window_id":42}}"#,
            r#"{"OverviewOpenedOrClosed":{"is_open":true}}"#,
            r#"{"WindowClosed":{}}"#,
        ] {
            assert!(event(untracked).is_err(), "{untracked}");
        }
    }
}
