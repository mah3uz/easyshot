#![allow(clippy::unwrap_used)]
#![cfg(feature = "wlroots")]

use es_backend::{Env, Probe, RuleSet, WindowList, wlroots};
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

#[test]
#[ignore = "needs headless sway: tests/harness/sway.sh run"]
fn platform_sway_rule_floats_the_editor() {
    let probe = harness();
    let Ok(rules) = (wlroots::RULES.build)(&probe) else {
        panic!("build failed");
    };
    let Ok(windows) = (wlroots::WINDOWS.build)(&probe) else {
        panic!("build failed");
    };
    rules.install(&RuleSet::default()).unwrap();
    swaymsg("exec alacritty --class dev.easyshot.editor");
    let info = wait_for(&*windows, "dev.easyshot.editor", |w| {
        w.floating && w.logical.is_some_and(|r| r.width() == 1180.0)
    });
    swaymsg("[app_id=dev.easyshot.editor] kill");
    eprintln!("editor rect {:?}", info.logical);
    assert!(info.floating, "the rule makes the editor float");
    assert_eq!(info.logical.map(|r| r.width()), Some(1180.0));
}
