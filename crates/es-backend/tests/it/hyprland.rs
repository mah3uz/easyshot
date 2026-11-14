#![cfg(feature = "hyprland")]

use es_backend::hyprland::{
    self, Client, HyprBinds, HyprEvent, HyprPointer, HyprWindows, Ipc, Monitor, Toplevels,
};
use es_backend::{BindHelper, BindSyntax, Env, Error, PointerQuery, Probe, Trigger, WindowList};
use es_geom::{Point, Rect};
use es_model::platform::{OutputId, WindowEvent, WindowId, WindowInfo};
use es_protocol::Route;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hyprland")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn clients() -> Vec<Client> {
    serde_json::from_slice(&fixture("clients.json")).expect("clients")
}

fn monitors() -> Vec<Monitor> {
    serde_json::from_slice(&fixture("monitors.json")).expect("monitors")
}

#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Recorded,
    Hyprlang,
    LuaError,
    Never,
}

/// Stands in for a Hyprland instance in a temporary directory, never the user's.
struct FakeHypr {
    dir: tempfile::TempDir,
    requests: Arc<Mutex<Vec<String>>>,
}

impl FakeHypr {
    fn start(answer: Answer) -> FakeHypr {
        let dir = tempfile::Builder::new()
            .prefix("es-hypr")
            .tempdir()
            .expect("tempdir");
        let listener = UnixListener::bind(dir.path().join(hyprland::REQUEST_SOCKET)).expect("bind");
        let replies: HashMap<&str, Vec<u8>> = HashMap::from([
            ("j/clients", fixture("clients.json")),
            ("j/monitors", fixture("monitors.json")),
            ("j/cursorpos", fixture("cursorpos.json")),
            ("j/binds", fixture("binds.json")),
        ]);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut buf = vec![0u8; 65536];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                log.lock().expect("log").push(request.clone());
                if answer == Answer::Never {
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                let lua = answer != Answer::Hyprlang;
                let reply = if request.starts_with("eval ") {
                    if lua {
                        b"ok".to_vec()
                    } else {
                        b"unknown request".to_vec()
                    }
                } else if answer == Answer::LuaError && request.starts_with("dispatch ") {
                    b"error: 3 [string \"return hl.dispatch(focuswindow address:0xddd4)\"]:1: ')' expected near 'address'".to_vec()
                } else if request.starts_with("dispatch hl.dsp.") && !lua {
                    b"invalid dispatcher".to_vec()
                } else if request.contains("address:0xddd4") {
                    b"ok".to_vec()
                } else if request.starts_with("dispatch ") {
                    b"Window not found".to_vec()
                } else {
                    replies.get(request.as_str()).cloned().unwrap_or_default()
                };
                let _ = stream.write_all(&reply);
            }
        });
        FakeHypr { dir, requests }
    }

    fn with_events(self, lines: Vec<u8>) -> FakeHypr {
        let listener =
            UnixListener::bind(self.dir.path().join(hyprland::EVENT_SOCKET)).expect("bind");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(&lines);
                std::thread::sleep(Duration::from_secs(5));
            }
        });
        self
    }

    fn ipc(&self) -> Ipc {
        Ipc::new(self.dir.path())
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("log").clone()
    }
}

fn ids(windows: &[WindowInfo]) -> Vec<&str> {
    windows.iter().map(|w| w.id.0.as_str()).collect()
}

#[test]
fn window_rects_are_global_logical_even_on_a_scaled_monitor() {
    let monitors = monitors();
    assert_eq!(
        monitors[1].logical(),
        Rect::new(2560.0, 0.0, 4480.0, 1200.0)
    );
    let windows = hyprland::visible_windows(&clients(), &monitors);
    let code = windows.iter().find(|w| w.id.0 == "0xccc3").expect("code");
    assert_eq!(code.logical, Some(Rect::new(2570.0, 10.0, 4470.0, 1190.0)));
    assert_eq!(code.output, Some(OutputId("eDP-1".into())));
    assert_eq!(code.pid, Some(4300));
}

#[test]
fn snapshot_is_topmost_first_and_skips_hidden_and_unshown_workspaces() {
    let windows = hyprland::visible_windows(&clients(), &monitors());
    assert_eq!(ids(&windows), ["0xbbb2", "0xccc3", "0xaaa1"]);
    let ranks: Vec<_> = windows.iter().map(|w| w.focus_rank).collect();
    assert_eq!(ranks, [Some(1), Some(0), Some(2)]);
    assert!(windows[0].floating);
}

#[test]
fn the_full_list_adds_windows_on_other_workspaces_without_geometry() {
    let windows = hyprland::all_windows(&clients(), &monitors());
    assert_eq!(ids(&windows), ["0xbbb2", "0xccc3", "0xaaa1", "0xddd4"]);
    let other = windows.iter().find(|w| w.id.0 == "0xddd4").expect("other");
    assert_eq!(other.workspace.as_deref(), Some("2"));
    assert_eq!(other.logical, None);
    assert_eq!(other.output, Some(OutputId("DP-1".into())));
}

#[test]
fn a_lua_config_is_sent_the_lua_dispatcher_and_a_refusal_is_reported() {
    assert_eq!(
        hyprland::focus_command("ddd4", true),
        "dispatch hl.dsp.focus({ window = 'address:0xddd4' })"
    );
    let fake = FakeHypr::start(Answer::Recorded).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    assert!(windows.can_focus());
    windows.focus(&WindowId("0xddd4".into())).expect("focus");
    let requests = fake.requests();
    assert!(requests.contains(&"dispatch hl.dsp.focus({ window = 'address:0xddd4' })".to_owned()));
    assert!(
        !requests
            .iter()
            .any(|r| r.starts_with("dispatch focuswindow"))
    );
    match windows.focus(&WindowId("0x9999".into())) {
        Err(Error::Protocol(m)) => assert!(m.contains("Window not found"), "{m}"),
        other => panic!("a refused focus must be reported, got {other:?}"),
    }
}

#[test]
fn a_hyprlang_config_is_sent_the_legacy_dispatcher_and_a_refusal_is_reported() {
    assert_eq!(
        hyprland::focus_command("ddd4", false),
        "dispatch focuswindow address:0xddd4"
    );
    let fake = FakeHypr::start(Answer::Hyprlang).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    windows.focus(&WindowId("0xddd4".into())).expect("focus");
    let requests = fake.requests();
    assert!(requests.contains(&"dispatch focuswindow address:0xddd4".to_owned()));
    assert!(!requests.iter().any(|r| r.starts_with("dispatch hl.dsp.")));
    match windows.focus(&WindowId("0x9999".into())) {
        Err(Error::Protocol(m)) => assert!(m.contains("Window not found"), "{m}"),
        other => panic!("a refused focus must be reported, got {other:?}"),
    }
}

#[test]
fn a_lua_error_reply_to_focus_is_an_error() {
    let fake = FakeHypr::start(Answer::LuaError).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    match windows.focus(&WindowId("0xddd4".into())) {
        Err(Error::Protocol(m)) => assert!(m.contains("')' expected"), "{m}"),
        other => panic!("a Lua error must be reported, got {other:?}"),
    }
}

#[test]
fn the_window_list_comes_from_the_clients_reply() {
    let fake = FakeHypr::start(Answer::Recorded).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    assert_eq!(windows.all().len(), 4);
}

// A window shot's scale is its pixels over this size, also for a window no screen shows.
#[test]
fn a_window_on_an_unshown_workspace_still_has_its_size() {
    let fake = FakeHypr::start(Answer::Recorded).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    let size = windows.size(&WindowId("0xddd4".into()));
    assert_eq!(size, Some(es_geom::Size::new(1260.0, 1420.0)));
    assert_eq!(windows.size(&WindowId("0x9999".into())), None);
}

#[test]
fn pointer_maps_to_the_output_under_it() {
    let fake = FakeHypr::start(Answer::Recorded);
    let pointer = HyprPointer { ipc: fake.ipc() };
    assert_eq!(pointer.pointer_global(), Some(Point::new(3000.0, 500.0)));
    assert_eq!(
        pointer.output_under_pointer(),
        Some(OutputId("eDP-1".into()))
    );
    assert_eq!(pointer.focused_output(), Some(OutputId("eDP-1".into())));
    let monitors = monitors();
    assert_eq!(
        hyprland::output_at(&monitors, Point::new(2559.0, 1439.0)),
        Some(OutputId("DP-1".into()))
    );
    assert_eq!(
        hyprland::output_at(&monitors, Point::new(2600.0, 1300.0)),
        None
    );
}

#[test]
fn a_compositor_that_never_answers_costs_one_timeout() {
    let fake = FakeHypr::start(Answer::Never);
    let started = Instant::now();
    let err = fake.ipc().request("j/cursorpos").expect_err("no answer");
    let took = started.elapsed();
    assert!(matches!(err, Error::Timeout), "{err:?}");
    assert!(took >= hyprland::REQUEST_TIMEOUT, "{took:?}");
    assert!(took < Duration::from_millis(500), "{took:?}");
    let pointer = HyprPointer { ipc: fake.ipc() };
    assert_eq!(pointer.pointer_global(), None);
}

#[test]
fn snippets_in_lua_and_hyprlang() {
    let binds = [
        (Trigger("SUPER+SHIFT+4".into()), Route::CaptureRegion),
        (Trigger("SUPER+SHIFT+O".into()), Route::Ocr),
    ];
    assert_eq!(
        hyprland::lua_snippet(&binds),
        "hl.bind(\"SUPER + SHIFT + 4\", hl.dsp.exec_cmd(\"easyshot capture region\"))\n\
         hl.bind(\"SUPER + SHIFT + O\", hl.dsp.exec_cmd(\"easyshot ocr\"))\n"
    );
    assert_eq!(
        hyprland::hyprlang_snippet(&binds),
        "bind = SUPER SHIFT, 4, exec, easyshot capture region\n\
         bind = SUPER SHIFT, O, exec, easyshot ocr\n"
    );
    let quoted = [(Trigger("SUPER+\"".into()), Route::CaptureRegion)];
    assert!(hyprland::lua_snippet(&quoted).starts_with(r#"hl.bind("SUPER + \"", "#));
}

#[test]
fn existing_binds_come_from_binds_json() {
    let fake = FakeHypr::start(Answer::Recorded);
    let helper = HyprBinds {
        ipc: fake.ipc(),
        syntax: BindSyntax::HyprLua,
        config: None,
        home: None,
    };
    let binds = helper.existing_binds().expect("binds");
    assert_eq!(
        binds,
        [
            (
                Trigger("SUPER+SHIFT+4".into()),
                "exec easyshot capture region".to_owned()
            ),
            (Trigger("SUPER+Q".into()), "killactive".to_owned()),
        ]
    );
    assert_eq!(fake.requests(), ["j/binds"]);
}

#[test]
fn socket2_lines_parse_into_window_events() {
    use HyprEvent::*;
    let parsed: Vec<_> = String::from_utf8(fixture("events.txt"))
        .expect("utf-8")
        .lines()
        .filter_map(hyprland::parse_event)
        .collect();
    assert_eq!(
        parsed,
        [
            Opened("0xccc3".into()),
            Focused("0xccc3".into()),
            Moved("0xbbb2".into()),
            Closed("0xaaa1".into()),
        ]
    );
}

fn toplevel(id: &str, app_id: &str, title: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: app_id.into(),
        title: title.into(),
        pid: None,
        logical: None,
        output: None,
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

fn fake_toplevels() -> Vec<WindowInfo> {
    vec![
        toplevel("tl-code", "code", "main.rs"),
        toplevel("tl-ff", "firefox", "Docs"),
        toplevel("tl-kitty-1", "kitty", "term"),
        toplevel("tl-kitty-2", "kitty", "term"),
    ]
}

#[test]
fn addresses_map_to_toplevels_only_when_unambiguous() {
    let joined = hyprland::join_toplevels(&clients(), &fake_toplevels());
    assert_eq!(joined.get("0xccc3"), Some(&WindowId("tl-code".into())));
    assert_eq!(joined.get("0xbbb2"), Some(&WindowId("tl-ff".into())));
    assert_eq!(joined.get("0xaaa1"), None);
}

#[test]
fn window_events_carry_the_same_ids_as_snapshots() {
    let fake = FakeHypr::start(Answer::Recorded).with_events(fixture("events.txt"));
    let source: Toplevels = Arc::new(fake_toplevels);
    let windows = HyprWindows::start(fake.ipc(), Some(source)).expect("start");
    let events = windows.events();
    let next = || {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(e) = events.try_recv() {
                return e;
            }
            assert!(Instant::now() < deadline, "no window event");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let WindowEvent::Opened(code) = next() else {
        panic!("expected Opened")
    };
    assert_eq!(code.id, WindowId("tl-code".into()));
    assert_eq!(code.logical, Some(Rect::new(2570.0, 10.0, 4470.0, 1190.0)));
    assert_eq!(next(), WindowEvent::Focused(WindowId("tl-code".into())));
    let WindowEvent::Changed(ff) = next() else {
        panic!("expected Changed")
    };
    assert_eq!(ff.id, WindowId("tl-ff".into()));
    assert_eq!(next(), WindowEvent::Closed(WindowId("0xaaa1".into())));
    assert!(windows.has_geometry());
    let snapshot = windows.try_snapshot().expect("snapshot");
    assert_eq!(ids(&snapshot), ["tl-ff", "tl-code", "0xaaa1"]);
}

#[test]
fn without_a_toplevel_source_ids_are_addresses() {
    let fake = FakeHypr::start(Answer::Recorded).with_events(Vec::new());
    let windows = HyprWindows::start(fake.ipc(), None).expect("start");
    assert_eq!(ids(&windows.snapshot()), ["0xbbb2", "0xccc3", "0xaaa1"]);
}

fn hypr_env(runtime: &Path, desktop: &str, signature: &str) -> Env {
    Env::from_vars([
        (hyprland::SIGNATURE_VAR, signature.to_owned()),
        ("XDG_RUNTIME_DIR", runtime.display().to_string()),
        ("XDG_CURRENT_DESKTOP", desktop.to_owned()),
    ])
}

#[test]
fn detection_needs_the_signature_and_a_hyprland_desktop() {
    let runtime = tempfile::tempdir().expect("tempdir");
    let env = hypr_env(runtime.path(), "Hyprland", "abc_1");
    assert_eq!(
        hyprland::socket_dir(&env),
        Some(runtime.path().join("hypr").join("abc_1"))
    );
    assert!(hyprland::socket_dir(&hypr_env(runtime.path(), "sway", "abc_1")).is_none());
    assert!(hyprland::socket_dir(&hypr_env(runtime.path(), "Hyprland", "../x")).is_none());
    assert!(hyprland::socket_dir(&Env::from_vars([("XDG_CURRENT_DESKTOP", "Hyprland")])).is_none());

    let probe = Probe {
        env,
        ..Probe::default()
    };
    assert_eq!((hyprland::POINTER.probe)(&probe), Some(30));
    let err = (hyprland::POINTER.build)(&probe)
        .err()
        .expect("no socket here");
    assert!(err.to_string().contains("no Hyprland socket"), "{err}");
}
