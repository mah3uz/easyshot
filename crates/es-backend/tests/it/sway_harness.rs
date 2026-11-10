#![allow(clippy::unwrap_used)]
#![cfg(feature = "wlroots")]

use es_backend::{Env, Probe, WindowList, wlroots};
use es_model::platform::{CaptureOpts, WindowInfo};
use std::process::Command;
use std::time::{Duration, Instant};

const SCALE: f64 = 1.5;

// These drive real windows, so they refuse anything but the headless sway harness.
fn harness() -> Probe {
    let env = Env::from_process();
    let runtime = env.var("XDG_RUNTIME_DIR").unwrap_or_default();
    assert!(
        env.var("EASYSHOT_TEST") == Some("1")
            && env.var("EASYSHOT_TEST_OUTPUT") == Some("HEADLESS-1")
            && env.var(wlroots::SOCKET_ENV).is_some()
            && runtime.contains("easyshot-harness-"),
        "run under tests/harness/sway.sh run -- cargo nextest run --run-ignored only"
    );
    Probe {
        env,
        ..Probe::default()
    }
}

fn swaymsg(command: &str) {
    let status = Command::new("swaymsg").arg(command).status().unwrap();
    assert!(status.success(), "swaymsg {command}");
}

// A window maps at its own size and shows sway's layout only once the client commits the configure.
fn wait_for(
    windows: &dyn WindowList,
    app_id: &str,
    settled: impl Fn(&WindowInfo) -> bool,
) -> WindowInfo {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let seen = windows.snapshot().into_iter().find(|w| w.app_id == app_id);
        match seen {
            Some(w) if settled(&w) || Instant::now() >= deadline => return w,
            None => assert!(Instant::now() < deadline, "{app_id} never mapped"),
            Some(_) => {}
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "needs headless sway: tests/harness/sway.sh run"]
fn platform_sway_tree_rect_matches_the_window_capture() {
    let probe = harness();
    let Ok(windows) = (wlroots::WINDOWS.build)(&probe) else {
        panic!("build failed");
    };
    let app_id = "es-harness-capture";
    swaymsg(&format!("exec alacritty --class {app_id}"));
    let info = wait_for(&*windows, app_id, |w| {
        w.logical
            .is_some_and(|r| (r.width() * SCALE, r.height() * SCALE) == (1920.0, 1080.0))
    });
    let rect = info.logical.unwrap();

    let conn = es_wayland::Connection::connect().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let toplevel = loop {
        if let Some(t) = conn.windows().into_iter().find(|t| t.app_id == app_id) {
            break t;
        }
        assert!(
            Instant::now() < deadline,
            "no foreign toplevel for {app_id}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    // Sway lays the window out before the client commits a buffer at the new size.
    let deadline = Instant::now() + Duration::from_secs(5);
    let (frame, captured) = loop {
        let frame = conn
            .capture_toplevel(&toplevel.id.0, CaptureOpts { cursor: false })
            .unwrap();
        let captured = (
            f64::from(frame.size_px.0) / frame.scale,
            f64::from(frame.size_px.1) / frame.scale,
        );
        if captured == (rect.width(), rect.height()) || Instant::now() >= deadline {
            break (frame, captured);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    swaymsg(&format!("[app_id={app_id}] kill"));

    eprintln!(
        "tree rect {rect:?}, capture {:?} at scale {} as {:?}",
        frame.size_px, frame.scale, frame.logical
    );
    assert_eq!(captured, (rect.width(), rect.height()));
    // Alone on its workspace it fills the 1920x1080 output, so a physical rect would be 1.5 times larger.
    assert_eq!(
        (rect.width() * SCALE, rect.height() * SCALE),
        (1920.0, 1080.0)
    );
}

// Sway gives the seat a keyboard, and so sends a keymap, only when it has an input device;
// the headless harness has none, so there the keymap stays absent.
#[test]
#[ignore = "needs headless sway: tests/harness/sway.sh run"]
fn platform_sway_client_without_focus_receives_the_keymap_when_the_seat_has_a_keyboard() {
    let _probe = harness();
    let inputs = Command::new("swaymsg")
        .args(["-t", "get_inputs"])
        .output()
        .unwrap();
    let has_keyboard = String::from_utf8_lossy(&inputs.stdout).contains("\"type\": \"keyboard\"");
    let conn = es_wayland::Connection::connect().unwrap();
    let deadline = Instant::now() + Duration::from_secs(if has_keyboard { 5 } else { 1 });
    let mut keymap = None;
    while keymap.is_none() && Instant::now() < deadline {
        keymap = conn.keymap();
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!(
        "seat keyboard: {has_keyboard}, keymap bytes: {:?}",
        keymap.as_ref().map(|k| k.text.len())
    );
    match keymap {
        Some(k) => {
            assert!(has_keyboard, "a keymap without a keyboard");
            assert!(k.text.contains("xkb_keymap"));
        }
        None => assert!(
            !has_keyboard,
            "the seat has a keyboard but no keymap arrived"
        ),
    }
}

fn distinct_frames(video: &std::path::Path) -> (usize, usize) {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(video)
        .args(["-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let sums: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.rsplit(',').next())
        .collect();
    let distinct: std::collections::HashSet<_> = sums.iter().collect();
    (sums.len(), distinct.len())
}

#[test]
#[ignore = "needs headless sway: tests/harness/sway.sh run"]
fn platform_sway_window_on_a_hidden_workspace_records_with_changing_frames() {
    let _probe = harness();
    let app_id = "es-harness-record";
    swaymsg(&format!(
        "exec alacritty --class {app_id} -e sh -c 'while :; do date +%N; sleep 0.05; done'"
    ));
    let conn = es_wayland::Connection::connect().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let window = loop {
        if let Some(w) = conn.windows().into_iter().find(|t| t.app_id == app_id) {
            break w;
        }
        assert!(
            Instant::now() < deadline,
            "no foreign toplevel for {app_id}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    swaymsg("workspace 2");

    let backend = es_backend::Backend::detect(&Probe::gather().unwrap());
    let recorder = backend
        .window_recorder
        .as_ref()
        .expect("a window recorder in headless sway");
    let dir = tempfile::tempdir().unwrap();
    let spec = es_record::RecordSpec {
        source: es_record::RecordSource::Window(window),
        fps: 30,
        codec: es_record::Codec::H264,
        container: es_record::Container::Mkv,
        quality: es_record::Quality::Medium,
        system_audio: false,
        mic: None,
        cursor: false,
        dir: dir.path().to_path_buf(),
    };
    let handle = recorder.start(spec).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let out = futures::executor::block_on(handle.stop()).unwrap();
    swaymsg(&format!("[app_id={app_id}] kill"));

    let (frames, distinct) = distinct_frames(&out.file);
    eprintln!("{frames} frames, {distinct} distinct");
    assert!(frames >= 40, "{frames} frames in two seconds at 30 fps");
    assert!(
        distinct >= 5,
        "only {distinct} distinct frames: the hidden window was not captured"
    );
}
