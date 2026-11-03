#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = Command::new("gdbus")
            .args(["call", "--session", "--dest", es_protocol::BUS_NAME_TEST])
            .args(["--object-path", es_protocol::OBJECT_PATH, "--method"])
            .arg(format!("{}.Command", es_protocol::INTERFACE))
            .args(["quit", "{}"])
            .output();
        let end = Instant::now() + Duration::from_secs(5);
        while Instant::now() < end && self.0.try_wait().ok().flatten().is_none() {
            sleep(Duration::from_millis(100));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn xdg(cmd: &mut Command, root: &Path) {
    for (var, dir) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
    ] {
        cmd.env(var, root.join(dir));
    }
    cmd.env("EASYSHOT_TEST_DIR", root.join("test"));
}

fn agent(root: &Path, what: &str) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.args(["agent", "list", what, "--json"]);
    xdg(&mut cmd, root);
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
    )
}

fn swaymsg(args: &[&str]) -> Value {
    let out = Command::new("swaymsg").args(args).output().unwrap();
    assert!(out.status.success(), "swaymsg {args:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn json_line(stdout: &str) -> Value {
    assert_eq!(stdout.lines().count(), 1, "one JSON object: {stdout:?}");
    serde_json::from_str(stdout).unwrap()
}

// Runs inside tests/harness/sway.sh, whose only output is HEADLESS-1; skipped anywhere else.
#[test]
fn platform_agent_lists_outputs_and_windows_as_sway_reports_them() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("daemon");
    xdg(&mut cmd, root.path());
    let log = std::fs::File::create(root.path().join("daemon.log")).unwrap();
    let _daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());

    let end = Instant::now() + Duration::from_secs(30);
    let (code, out) = loop {
        let (code, out) = agent(root.path(), "outputs");
        if code != 3 || Instant::now() > end {
            break (code, out);
        }
        sleep(Duration::from_millis(200));
    };
    assert_eq!(code, 0, "{out}");
    let reply = json_line(&out);
    let sway = swaymsg(&["-t", "get_outputs", "-r"]);
    let rect = &sway[0]["rect"];
    assert_eq!(reply["ok"], Value::Bool(true));
    assert_eq!(reply["kind"], "outputs");
    let outputs = reply["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 1, "{reply}");
    let o = &outputs[0];
    assert_eq!(o["name"], "HEADLESS-1");
    for key in ["x", "y", "width", "height"] {
        assert_eq!(o[key], rect[key], "{key}: {o} against {rect}");
    }
    assert_eq!(o["scale"], 1.5);

    let mut gallery = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    gallery.arg("gallery");
    xdg(&mut gallery, root.path());
    assert!(gallery.status().unwrap().success());
    let end = Instant::now() + Duration::from_secs(30);
    let windows = loop {
        let (code, out) = agent(root.path(), "windows");
        assert_eq!(code, 0, "{out}");
        let reply = json_line(&out);
        assert_eq!(reply["kind"], "windows");
        let windows = reply["windows"].as_array().unwrap().clone();
        if !windows.is_empty() || Instant::now() > end {
            break windows;
        }
        sleep(Duration::from_millis(200));
    };
    assert_eq!(windows.len(), 1, "the Home window: {windows:?}");
    let w = &windows[0];
    let tree = swaymsg(&["-t", "get_tree", "-r"]).to_string();
    let id = w["id"].as_str().unwrap();
    assert!(
        tree.contains(&format!("\"id\":{id},")),
        "sway has window {id}"
    );
    assert!(tree.contains(&format!("\"app_id\":{}", w["app_id"])), "{w}");
    assert_eq!(w["output"], "HEADLESS-1");
    assert_eq!(w["focused"], true);
    for key in ["x", "y", "width", "height"] {
        assert!(w[key].is_i64(), "{key} in {w}");
    }
}
