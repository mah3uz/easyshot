#![allow(clippy::unwrap_used)]
#![cfg(feature = "niri")]

use es_backend::{BackendError, BindSyntax, Env, Error, Probe, Trigger, niri};
use es_geom::Rect;
use es_model::platform::{OutputId, WindowEvent, WindowId, WindowInfo};
use es_protocol::Route;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const OUTPUTS: &str = include_str!("../fixtures/niri/outputs.json");
const WORKSPACES: &str = include_str!("../fixtures/niri/workspaces.json");
const WINDOWS: &str = include_str!("../fixtures/niri/windows.json");

#[derive(Default)]
struct Shared {
    subscribers: Vec<UnixStream>,
}

struct FakeNiri {
    _dir: tempfile::TempDir,
    path: PathBuf,
    shared: Arc<Mutex<Shared>>,
}

fn reply(request: &Value) -> Value {
    let parse = |s: &str| serde_json::from_str::<Value>(s).unwrap();
    let ok = match request.as_str() {
        Some("Outputs") => json!({ "Outputs": parse(OUTPUTS) }),
        Some("Workspaces") => json!({ "Workspaces": parse(WORKSPACES) }),
        Some("Windows") => json!({ "Windows": parse(WINDOWS) }),
        Some("FocusedOutput") => json!({ "FocusedOutput": parse(OUTPUTS)["DP-1"] }),
        Some("EventStream") => json!("Handled"),
        _ => return json!({ "Err": "unsupported" }),
    };
    json!({ "Ok": ok })
}

impl FakeNiri {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("niri.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let s = shared.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let mut conn = conn.unwrap();
                let s = s.clone();
                std::thread::spawn(move || {
                    let mut line = String::new();
                    BufReader::new(conn.try_clone().unwrap())
                        .read_line(&mut line)
                        .unwrap();
                    let request: Value = serde_json::from_str(&line).unwrap();
                    conn.write_all(format!("{}\n", reply(&request)).as_bytes())
                        .unwrap();
                    if request == "EventStream" {
                        s.lock().unwrap().subscribers.push(conn);
                    }
                });
            }
        });
        FakeNiri {
            _dir: dir,
            path,
            shared,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            env: Env::from_vars([(niri::SOCKET_ENV, self.path.to_str().unwrap())]),
            ..Probe::default()
        }
    }

    fn push(&self, event: &Value) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.shared.lock().unwrap().subscribers.is_empty() {
            assert!(Instant::now() < deadline, "no event stream");
            std::thread::sleep(Duration::from_millis(5));
        }
        let line = format!("{event}\n");
        for s in &mut self.shared.lock().unwrap().subscribers {
            s.write_all(line.as_bytes()).unwrap();
        }
    }
}

fn find<'a>(ws: &'a [WindowInfo], id: &str) -> &'a WindowInfo {
    ws.iter().find(|w| w.id.0 == id).unwrap()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Option<Rect> {
    Some(Rect::new(x0, y0, x1, y1))
}

fn drain(rx: &async_channel::Receiver<WindowEvent>, n: usize) -> Vec<WindowEvent> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut out = Vec::new();
    while out.len() < n {
        match rx.try_recv() {
            Ok(e) => out.push(e),
            Err(_) => {
                assert!(Instant::now() < deadline, "got only {out:?}");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    std::thread::sleep(Duration::from_millis(50));
    assert!(rx.try_recv().is_err(), "more events than expected");
    out
}

fn window(id: u64) -> Value {
    serde_json::from_str::<Vec<Value>>(WINDOWS)
        .unwrap()
        .into_iter()
        .find(|w| w["id"] == id)
        .unwrap()
}

#[test]
fn niri_rects_derive_from_tiles_in_a_scrolled_workspace() {
    let fake = FakeNiri::start();
    let Ok(windows) = (niri::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let ws = windows.snapshot();
    assert_eq!(ws.len(), 6);
    // Scrolled half off the left edge: only the visible part is a target.
    assert_eq!(find(&ws, "10").logical, rect(0.0, 2.0, 398.0, 718.0));
    assert_eq!(find(&ws, "11").logical, rect(402.0, 2.0, 1198.0, 718.0));
    assert_eq!(find(&ws, "12").logical, rect(1202.0, 2.0, 1280.0, 718.0));
    assert_eq!(find(&ws, "13").logical, None, "scrolled out of view");
    assert_eq!(find(&ws, "14").logical, None, "on an inactive workspace");
    let floating = find(&ws, "15");
    // The second output starts at x 1280 in the global logical space.
    assert_eq!(floating.logical, rect(1380.5, 100.0, 2020.5, 580.0));
    assert!(floating.floating);
    assert_eq!(floating.output, Some(OutputId("HDMI-A-1".into())));
    assert_eq!(find(&ws, "14").workspace.as_deref(), Some("mail"));
    assert_eq!(find(&ws, "11").workspace.as_deref(), Some("1"));
    assert_eq!(find(&ws, "11").pid, Some(1011));
    assert!(windows.has_geometry());
}

#[test]
fn niri_focus_rank_puts_the_focused_window_first() {
    let fake = FakeNiri::start();
    let Ok(windows) = (niri::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let mut ws = windows.snapshot();
    ws.sort_by_key(|w| w.focus_rank);
    let order: Vec<&str> = ws.iter().map(|w| w.id.0.as_str()).collect();
    assert_eq!(order, ["11", "15", "10", "12", "13", "14"]);
}

#[test]
fn niri_events_map_to_window_events() {
    let fake = FakeNiri::start();
    let Ok(windows) = (niri::WINDOWS.build)(&fake.probe()) else {
        panic!("build failed");
    };
    let rx = windows.events();

    fake.push(&json!({ "SomethingFromANewerNiri": {} }));
    let mut scrolled = window(10)["layout"].clone();
    scrolled["tile_pos_in_workspace_view"] = json!([0.0, 0.0]);
    fake.push(&json!({ "WindowLayoutsChanged": { "changes": [[10, scrolled]] } }));
    match &drain(&rx, 1)[..] {
        [WindowEvent::Changed(w)] => assert_eq!(w.logical, rect(2.0, 2.0, 798.0, 718.0)),
        other => panic!("expected one Changed, got {other:?}"),
    }

    fake.push(&json!({ "WindowFocusChanged": { "id": 10 } }));
    assert_eq!(drain(&rx, 1), [WindowEvent::Focused(WindowId("10".into()))]);

    let mut opened = window(13);
    opened["id"] = json!(16);
    fake.push(&json!({ "WindowOpenedOrChanged": { "window": opened } }));
    assert!(matches!(&drain(&rx, 1)[..], [WindowEvent::Opened(w)] if w.id.0 == "16"));

    fake.push(&json!({ "WindowClosed": { "id": 16 } }));
    assert_eq!(drain(&rx, 1), [WindowEvent::Closed(WindowId("16".into()))]);

    fake.push(&json!({ "WorkspaceActivated": { "id": 2, "focused": true } }));
    let mut changed: Vec<(String, Option<Rect>)> = drain(&rx, 4)
        .into_iter()
        .map(|e| match e {
            WindowEvent::Changed(w) => (w.id.0, w.logical),
            other => panic!("expected Changed, got {other:?}"),
        })
        .collect();
    changed.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        changed,
        [
            ("10".to_owned(), None),
            ("11".to_owned(), None),
            ("12".to_owned(), None),
            ("14".to_owned(), rect(0.0, 0.0, 1280.0, 720.0)),
        ]
    );
}

#[test]
fn niri_requests_time_out_after_fifty_milliseconds() {
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
        env: Env::from_vars([(niri::SOCKET_ENV, path.to_str().unwrap())]),
        ..Probe::default()
    };
    let started = Instant::now();
    let result = (niri::WINDOWS.build)(&probe);
    let took = started.elapsed();
    assert!(matches!(
        result.err(),
        Some(BackendError::Backend(Error::Timeout))
    ));
    assert!(took >= Duration::from_millis(50), "{took:?}");
    assert!(took < Duration::from_millis(250), "{took:?}");
    assert!(matches!(
        (niri::POINTER.build)(&probe).err(),
        Some(BackendError::Backend(Error::Timeout))
    ));
}

#[test]
fn niri_focused_output_stands_in_for_the_pointer() {
    let fake = FakeNiri::start();
    let Ok(pointer) = (niri::POINTER.build)(&fake.probe()) else {
        panic!("build failed");
    };
    assert_eq!(pointer.focused_output(), Some(OutputId("DP-1".into())));
    assert_eq!(
        pointer.output_under_pointer(),
        Some(OutputId("DP-1".into()))
    );
    assert_eq!(pointer.pointer_global(), None);
}

#[test]
fn niri_snippet_is_kdl_and_existing_binds_come_from_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    let probe = Probe {
        env: Env::from_vars([
            (niri::SOCKET_ENV, "/nonexistent"),
            ("NIRI_CONFIG", config.to_str().unwrap()),
        ]),
        ..Probe::default()
    };
    let Ok(binds) = (niri::BINDS.build)(&probe) else {
        panic!("build failed");
    };
    assert_eq!(binds.syntax(), BindSyntax::NiriKdl);
    assert_eq!(
        binds.snippet(&[
            (Trigger("Mod+Shift+4".into()), Route::CaptureRegion),
            (Trigger("Mod+O".into()), Route::Open("/tmp/a.png".into())),
            (Trigger("Mod+Shift+O".into()), Route::Ocr),
        ]),
        "// Paste inside your binds { } block\n\
         Mod+Shift+4 { spawn \"easyshot\" \"capture\" \"region\"; }\n\
         Mod+Shift+O { spawn \"easyshot\" \"ocr\"; }\n"
    );
    assert!(matches!(binds.existing_binds(), Err(Error::NotFound(_))));

    std::fs::write(
        &config,
        r#"input {
    keyboard { xkb { layout "us"; } }
}
binds {
    // Mod+X { spawn "nothing"; }
    Mod+T hotkey-overlay-title="Open a Terminal" { spawn "alacritty"; }
    Mod+Shift+4 { spawn "easyshot" "capture" "region"; }
    /-Mod+Y { spawn "disabled"; }
    Mod+Shift+E {
        quit;
    }
    Print { spawn "sh" "-c" "grim // {not a brace}"; }
}
"#,
    )
    .unwrap();
    let found: Vec<(String, String)> = binds
        .existing_binds()
        .unwrap()
        .into_iter()
        .map(|(t, c)| (t.0, c))
        .collect();
    let expected: Vec<(String, String)> = [
        ("Mod+T", r#"spawn "alacritty""#),
        ("Mod+Shift+4", r#"spawn "easyshot" "capture" "region""#),
        ("Mod+Shift+E", "quit"),
        ("Print", r#"spawn "sh" "-c" "grim // {not a brace}""#),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
    .collect();
    assert_eq!(found, expected);
}

#[test]
fn niri_is_detected_from_its_socket() {
    let probe = |vars: &[(&str, &str)]| Probe {
        env: Env::from_vars(vars.iter().copied()),
        ..Probe::default()
    };
    assert_eq!((niri::WINDOWS.probe)(&probe(&[])), None);
    let sock = (niri::SOCKET_ENV, "/run/user/1000/niri.sock");
    let plain = (niri::WINDOWS.probe)(&probe(&[sock]));
    let named = (niri::WINDOWS.probe)(&probe(&[sock, ("XDG_CURRENT_DESKTOP", "niri")]));
    assert!(plain.is_some() && named > plain);
}
