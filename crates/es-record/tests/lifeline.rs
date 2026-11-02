#![allow(clippy::unwrap_used)]
mod common;

use common::*;
use es_record::{GsrRecorder, Recorder};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const DAEMON_DIR: &str = "ES_FAKE_DAEMON_DIR";

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The stand-in daemon: only acts when re-executed by the test below.
#[test]
fn fake_daemon() {
    let Some(dir) = std::env::var_os(DAEMON_DIR) else {
        return;
    };
    let dir = Path::new(&dir);
    let handle = GsrRecorder::native()
        .trust_kms()
        .with_program(fake("fake-gsr"))
        .with_ipc_dir(dir)
        .with_env("FAKE_LOG", &dir.join("log").to_string_lossy())
        .with_env("FAKE_MODE", "no_ipc")
        .start(spec(dir, output("DP-1")))
        .unwrap();
    std::fs::write(dir.join("started"), "").unwrap();
    std::thread::sleep(Duration::from_secs(60));
    drop(handle);
}

#[test]
fn the_recorder_stops_when_the_daemon_is_killed() {
    let tmp = tempfile::tempdir().unwrap();
    let mut daemon = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fake_daemon", "--nocapture"])
        .env(DAEMON_DIR, tmp.path())
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .spawn()
        .unwrap();
    wait_for("the recorder to start", || {
        tmp.path().join("started").exists()
    });
    let log = tmp.path().join("log");
    let pid = pid_in(&read_log(&log));

    daemon.kill().unwrap();
    daemon.wait().unwrap();

    wait_for("the recorder to be interrupted", || {
        read_log(&log).iter().any(|l| l == "finish sigint")
    });
    wait_for("the recorder to exit", || process_gone(pid));
}
