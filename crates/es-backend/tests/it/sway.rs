#![allow(clippy::unwrap_used)]
#![cfg(feature = "wlroots")]

use es_backend::{BackendError, BindSyntax, Env, Error, Probe, Trigger, wlroots};
use es_geom::Rect;
use es_model::platform::{OutputId, WindowEvent, WindowId, WindowInfo};
use es_protocol::{Route, SettingsPage};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TREE: &str = include_str!("../fixtures/sway/tree.json");
const OUTPUTS: &str = include_str!("../fixtures/sway/outputs.json");
const EVENTS: &str = include_str!("../fixtures/sway/events.jsonl");

fn frame(kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut m = b"i3-ipc".to_vec();
    m.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_ne_bytes());
    m.extend_from_slice(&kind.to_ne_bytes());
    m.extend_from_slice(payload);
    m
}

#[derive(Default)]
struct Shared {
    tree: String,
    config: String,
    command_reply: String,
    commands: Vec<String>,
    subscribers: Vec<UnixStream>,
}

struct FakeSway {
    _dir: tempfile::TempDir,
    path: PathBuf,
    shared: Arc<Mutex<Shared>>,
}

impl FakeSway {
    fn start(tree: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sway-ipc.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let shared = Arc::new(Mutex::new(Shared {
            tree: tree.to_owned(),
            command_reply: r#"[{"success":true}]"#.to_owned(),
            ..Shared::default()
        }));
        let s = shared.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let s = s.clone();
                std::thread::spawn(move || serve(conn.unwrap(), &s));
            }
        });
        FakeSway {
            _dir: dir,
            path,
            shared,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            env: Env::from_vars([
                (wlroots::SOCKET_ENV, self.path.to_str().unwrap()),
                ("XDG_CURRENT_DESKTOP", "sway"),
            ]),
            ..Probe::default()
        }
    }

    fn set_tree(&self, tree: &Value) {
        self.shared.lock().unwrap().tree = tree.to_string();
    }

    fn push_window_event(&self, payload: &str) {
        let msg = frame(0x8000_0003, payload.as_bytes());
        for s in &mut self.shared.lock().unwrap().subscribers {
            s.write_all(&msg).unwrap();
        }
    }
}

fn serve(mut conn: UnixStream, shared: &Mutex<Shared>) {
    loop {
        let mut header = [0u8; 14];
        if conn.read_exact(&mut header).is_err() {
            return;
        }
        assert_eq!(&header[..6], b"i3-ipc");
        let len = u32::from_ne_bytes(header[6..10].try_into().unwrap()) as usize;
        let kind = u32::from_ne_bytes(header[10..14].try_into().unwrap());
        let mut payload = vec![0u8; len];
        conn.read_exact(&mut payload).unwrap();
        let mut s = shared.lock().unwrap();
        let reply = match kind {
            0 => {
                s.commands.push(String::from_utf8(payload).unwrap());
                s.command_reply.clone()
            }
            2 => {
                conn.write_all(&frame(2, br#"{"success":true}"#)).unwrap();
                s.subscribers.push(conn.try_clone().unwrap());
                continue;
            }
            3 => OUTPUTS.to_owned(),
            4 => s.tree.clone(),
            9 => serde_json::json!({ "config": s.config }).to_string(),
            other => panic!("unexpected request {other}"),
        };
        conn.write_all(&frame(kind, reply.as_bytes())).unwrap();
    }
}

fn tree() -> Value {
    serde_json::from_str(TREE).unwrap()
}

fn workspace(t: &mut Value) -> &mut Value {
    &mut t["nodes"][1]["nodes"][0]
}

fn find<'a>(ws: &'a [WindowInfo], id: &str) -> &'a WindowInfo {
    ws.iter().find(|w| w.id.0 == id).unwrap()
}

fn recv(rx: &async_channel::Receiver<WindowEvent>) -> WindowEvent {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(e) = rx.try_recv() {
            return e;
        }
        assert!(Instant::now() < deadline, "no window event");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn sway_rects_are_logical_at_scale_one_and_a_half() {
    let fake = FakeSway::start(TREE);
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let ws = windows.snapshot();
    assert_eq!(ws.len(), 2, "the scratchpad and containers are not windows");
    let tiled = find(&ws, "318879107658909f610dcaefc3da81c6");
    // A 1920x1080 output at scale 1.5 is 1280x720 logical.
    assert_eq!(tiled.logical, Some(Rect::new(0.0, 0.0, 1280.0, 720.0)));
    assert_eq!(tiled.app_id, "term-a");
    assert_eq!(tiled.output, Some(OutputId("HEADLESS-1".into())));
    assert_eq!(tiled.workspace.as_deref(), Some("1"));
    assert!(!tiled.floating);
    let floating = find(&ws, "02abdeb54c23384b1188aa6059bdef1f");
    assert_eq!(floating.logical, Some(Rect::new(100.0, 50.0, 700.0, 450.0)));
    assert!(floating.floating);
    assert!(windows.has_geometry());
}

#[test]
fn sway_content_rect_excludes_borders_and_hidden_windows_have_none() {
    let mut t = tree();
    let tiled = &mut workspace(&mut t)["nodes"][0];
    tiled["rect"] = serde_json::json!({"x": 10, "y": 20, "width": 400, "height": 300});
    tiled["window_rect"] = serde_json::json!({"x": 2, "y": 2, "width": 396, "height": 296});
    workspace(&mut t)["floating_nodes"][0]["visible"] = Value::Bool(false);
    let fake = FakeSway::start(&t.to_string());
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let ws = windows.snapshot();
    assert_eq!(
        find(&ws, "318879107658909f610dcaefc3da81c6").logical,
        Some(Rect::new(12.0, 22.0, 408.0, 318.0))
    );
    assert_eq!(find(&ws, "02abdeb54c23384b1188aa6059bdef1f").logical, None);
}

fn con(id: u64, app: &str, visible: bool) -> Value {
    serde_json::json!({
        "id": id, "type": "con", "name": app, "app_id": app, "pid": id, "visible": visible,
        "rect": {"x": 0, "y": 0, "width": 800, "height": 600},
        "window_rect": {"x": 0, "y": 0, "width": 800, "height": 600},
        "nodes": [], "floating_nodes": [], "focus": []
    })
}

fn tree_with_other_workspace_and_scratchpad() -> Value {
    let mut t = tree();
    t["nodes"][1]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": 40, "type": "workspace", "name": "2", "nodes": [con(41, "mail", false)],
            "floating_nodes": [], "focus": [41]
        }));
    t["nodes"][0]["nodes"][0]["floating_nodes"] = serde_json::json!([con(50, "dropdown", false)]);
    t
}

#[test]
fn sway_full_list_has_other_workspaces_and_leaves_out_the_scratchpad() {
    let fake = FakeSway::start(&tree_with_other_workspace_and_scratchpad().to_string());
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let all = windows.all();
    let mut ids: Vec<_> = all.iter().map(|w| w.id.0.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "02abdeb54c23384b1188aa6059bdef1f",
            "318879107658909f610dcaefc3da81c6",
            "41"
        ]
    );
    let mail = find(&all, "41");
    assert_eq!(mail.workspace.as_deref(), Some("2"));
    assert_eq!(mail.logical, None);
}

#[test]
fn sway_focus_runs_con_id_focus_and_reports_a_failure() {
    let fake = FakeSway::start(TREE);
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    assert!(windows.can_focus());
    windows.focus(&WindowId("6".into())).unwrap();
    assert_eq!(fake.shared.lock().unwrap().commands, ["[con_id=6] focus"]);
    fake.shared.lock().unwrap().command_reply =
        r#"[{"success":false,"error":"No relevant containers"}]"#.to_owned();
    match windows.focus(&WindowId("99".into())) {
        Err(Error::Protocol(m)) => assert_eq!(m, "No relevant containers"),
        other => panic!("a failed focus must be reported, got {other:?}"),
    }
    assert!(windows.focus(&WindowId("not-a-number".into())).is_err());
}

// Window capture finds a window by this identifier; a con id would match no toplevel.
#[test]
fn sway_names_a_window_by_its_foreign_toplevel_identifier_and_focuses_it_by_con_id() {
    let mut t = tree_with_other_workspace_and_scratchpad();
    t["nodes"][1]["nodes"][1]["nodes"][0]["foreign_toplevel_identifier"] =
        Value::from("ec6343ae58326e4fffd959672365ad9f");
    let fake = FakeSway::start(&t.to_string());
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let all = windows.all();
    let mail = find(&all, "ec6343ae58326e4fffd959672365ad9f");
    assert_eq!(mail.app_id, "mail");
    assert!(all.iter().all(|w| w.id.0 != "41"), "{all:?}");
    assert_eq!(mail.logical, None);
    assert_eq!(
        windows.size(&mail.id),
        Some(es_geom::Size::new(800.0, 600.0)),
        "a hidden window's shot is sized by its layout"
    );
    windows.focus(&mail.id).unwrap();
    assert_eq!(fake.shared.lock().unwrap().commands, ["[con_id=41] focus"]);

    let rx = windows.events();
    fake.push_window_event(
        r#"{"change":"focus","container":{"id":41,"foreign_toplevel_identifier":"ec6343ae58326e4fffd959672365ad9f"}}"#,
    );
    assert_eq!(recv(&rx), WindowEvent::Focused(mail.id.clone()));
}

#[test]
fn sway_focus_order_follows_the_focus_lists() {
    let mut t = tree();
    let fake = FakeSway::start(&t.to_string());
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let ws = windows.snapshot();
    assert_eq!(
        find(&ws, "318879107658909f610dcaefc3da81c6").focus_rank,
        Some(0)
    );
    assert_eq!(
        find(&ws, "02abdeb54c23384b1188aa6059bdef1f").focus_rank,
        Some(1)
    );
    workspace(&mut t)["focus"] = serde_json::json!([6, 5]);
    fake.set_tree(&t);
    let ws = windows.snapshot();
    assert_eq!(
        find(&ws, "02abdeb54c23384b1188aa6059bdef1f").focus_rank,
        Some(0)
    );
    assert_eq!(
        find(&ws, "318879107658909f610dcaefc3da81c6").focus_rank,
        Some(1)
    );
}

#[test]
fn sway_window_events_map_to_window_events() {
    let lines: Vec<&str> = EVENTS.lines().collect();
    let full = tree();
    let mut without = tree();
    workspace(&mut without)["floating_nodes"] = Value::Array(vec![]);
    workspace(&mut without)["focus"] = serde_json::json!([5]);
    let fake = FakeSway::start(&without.to_string());
    let Ok(windows) = (wlroots::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let rx = windows.events();

    fake.set_tree(&full);
    fake.push_window_event(lines[4]);
    match recv(&rx) {
        WindowEvent::Opened(w) => {
            assert_eq!(w.id, WindowId("02abdeb54c23384b1188aa6059bdef1f".into()))
        }
        other => panic!("expected Opened, got {other:?}"),
    }

    fake.push_window_event(lines[6]);
    assert_eq!(
        recv(&rx),
        WindowEvent::Focused(WindowId("02abdeb54c23384b1188aa6059bdef1f".into()))
    );

    let mut renamed = full.clone();
    workspace(&mut renamed)["floating_nodes"][0]["name"] = Value::from("renamed");
    fake.set_tree(&renamed);
    fake.push_window_event(lines[7]);
    match recv(&rx) {
        WindowEvent::Changed(w) => assert_eq!(w.title, "renamed"),
        other => panic!("expected Changed, got {other:?}"),
    }

    fake.set_tree(&without);
    fake.push_window_event(r#"{"change":"close","container":{"id":6}}"#);
    assert_eq!(
        recv(&rx),
        WindowEvent::Closed(WindowId("02abdeb54c23384b1188aa6059bdef1f".into()))
    );
    assert!(rx.try_recv().is_err(), "nothing else was reported");
}

#[test]
fn sway_focused_output_stands_in_for_the_pointer() {
    let fake = FakeSway::start(TREE);
    let Ok(pointer) = (wlroots::POINTER.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let headless = Some(OutputId("HEADLESS-1".into()));
    assert_eq!(pointer.focused_output(), headless);
    assert_eq!(pointer.output_under_pointer(), headless);
    assert_eq!(pointer.pointer_global(), None);
}

#[test]
fn sway_requests_time_out_after_fifty_milliseconds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("silent.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let held = Arc::new(Mutex::new(Vec::new()));
    let h = held.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            h.lock().unwrap().push(conn.unwrap());
        }
    });
    let probe = Probe {
        env: Env::from_vars([(wlroots::SOCKET_ENV, path.to_str().unwrap())]),
        ..Probe::default()
    };
    let started = Instant::now();
    let result = (wlroots::WINDOWS.build)(&probe);
    let took = started.elapsed();
    assert!(matches!(
        result.err(),
        Some(BackendError::Backend(Error::Timeout))
    ));
    assert!(took >= Duration::from_millis(50), "{took:?}");
    assert!(took < Duration::from_millis(250), "{took:?}");
    assert!(matches!(
        (wlroots::POINTER.build)(&probe).err(),
        Some(BackendError::Backend(Error::Timeout))
    ));
}

#[test]
fn sway_snippet_and_existing_binds() {
    let fake = FakeSway::start(TREE);
    let Ok(binds) = (wlroots::BINDS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    assert_eq!(binds.syntax(), BindSyntax::Sway);
    let snippet = binds.snippet(&[
        (Trigger("$mod+Shift+4".into()), Route::CaptureRegion),
        (Trigger("$mod+o".into()), Route::Open("/tmp/a.png".into())),
        (
            Trigger("$mod+comma".into()),
            Route::Settings(Some(SettingsPage::Shortcuts)),
        ),
    ]);
    assert_eq!(
        snippet,
        "bindsym $mod+Shift+4 exec easyshot capture region\n\
         bindsym $mod+comma exec easyshot settings shortcuts\n"
    );
    fake.shared.lock().unwrap().config = "set $mod Mod4\n\
        # bindsym $mod+x exec nothing\n\
        bindsym $mod+Shift+4 exec easyshot capture region\n\
        bindsym --release --no-repeat Print exec grim\n\
        bindsym {\n    $mod+1 workspace number 1\n}\n\
        mode \"resize\" {\n    bindsym h resize shrink width 10px\n}\n\
        bindcode 107 exec easyshot capture fullscreen\n"
        .to_owned();
    let found: Vec<(String, String)> = binds
        .existing_binds()
        .unwrap()
        .into_iter()
        .map(|(t, c)| (t.0, c))
        .collect();
    let pairs = |v: &[(&str, &str)]| -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    };
    assert_eq!(
        found,
        pairs(&[
            ("$mod+Shift+4", "exec easyshot capture region"),
            ("Print", "exec grim"),
            ("$mod+1", "workspace number 1"),
            ("107", "exec easyshot capture fullscreen"),
        ])
    );
}

#[test]
fn sway_is_detected_from_its_socket() {
    let probe = |vars: &[(&str, &str)]| Probe {
        env: Env::from_vars(vars.iter().copied()),
        ..Probe::default()
    };
    assert_eq!((wlroots::WINDOWS.probe)(&probe(&[])), None);
    assert_eq!(
        (wlroots::WINDOWS.probe)(&probe(&[("XDG_CURRENT_DESKTOP", "sway")])),
        None
    );
    let sock = (wlroots::SOCKET_ENV, "/run/user/1000/sway-ipc.sock");
    let plain = (wlroots::WINDOWS.probe)(&probe(&[sock]));
    let named = (wlroots::WINDOWS.probe)(&probe(&[sock, ("XDG_CURRENT_DESKTOP", "X:sway")]));
    assert!(plain.is_some() && named > plain);
}

#[test]
fn reload_runs_sways_reload_command_and_reports_a_failure() {
    let fake = FakeSway::start(TREE);
    let Ok(binds) = (wlroots::BINDS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    binds.reload().unwrap();
    assert_eq!(fake.shared.lock().unwrap().commands, ["reload"]);
    fake.shared.lock().unwrap().command_reply =
        r#"[{"success":false,"error":"Config file not found"}]"#.to_owned();
    match binds.reload() {
        Err(Error::Protocol(m)) => assert_eq!(m, "Config file not found"),
        other => panic!("a failed reload must be reported, got {other:?}"),
    }
}
