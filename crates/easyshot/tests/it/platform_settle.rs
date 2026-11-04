#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEST: &str = es_protocol::BUS_NAME_TEST;
const PATH: &str = es_protocol::OBJECT_PATH;
const OUTPUT: &str = "easyshot-test";

fn gdbus(method: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("gdbus")
        .args(["call", "--session", "--dest", DEST, "--object-path", PATH])
        .args(["--method", method])
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn command(route: &str) -> Option<String> {
    gdbus(
        &format!("{}.Command", es_protocol::INTERFACE),
        &[route, "{}"],
    )
}

fn until<T>(secs: u64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if let Some(v) = f() {
            return Some(v);
        }
        sleep(Duration::from_millis(100));
    }
    None
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = command("quit");
        if until(5, || self.0.try_wait().ok().flatten()).is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn start_daemon(root: &Path) -> Daemon {
    let child = Command::new(env!("CARGO_BIN_EXE_easyshot"))
        .arg("daemon")
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("EASYSHOT_TEST_DIR", root.join("test"))
        .env("EASYSHOT_LOG", "info")
        .stderr(Stdio::from(
            std::fs::File::create(root.join("daemon.log")).unwrap(),
        ))
        .spawn()
        .unwrap();
    Daemon(child)
}

fn screen(root: &Path, name: &str) -> image::RgbaImage {
    let file = root.join(format!("{name}.png"));
    let status = Command::new("grim")
        .args(["-o", OUTPUT])
        .arg(&file)
        .status()
        .unwrap();
    assert!(status.success(), "grim");
    if let Some(dir) = std::env::var_os("EASYSHOT_E2E_SHOT") {
        let _ = std::fs::copy(&file, Path::new(&dir).join(format!("settle-{name}.png")));
    }
    image::open(&file).unwrap().to_rgba8()
}

fn differs(a: &image::Rgba<u8>, b: &image::Rgba<u8>) -> bool {
    a.0.iter().zip(b.0).any(|(x, y)| x.abs_diff(y) > 8)
}

/// Of the pixels the bar covered, how many still differ from the empty screen.
fn bar_pixels(empty: &image::RgbaImage, bar: &image::RgbaImage, now: &image::RgbaImage) -> usize {
    empty
        .pixels()
        .zip(bar.pixels())
        .zip(now.pixels())
        .filter(|((e, b), n)| differs(e, b) && differs(e, n))
        .count()
}

// Runs inside tests/harness/hypr.sh, whose only output is easyshot-test, under the session's own
// animation settings; skipped anywhere else.
#[test]
fn platform_hypr_the_frozen_frame_after_the_bar_closes_has_no_bar_in_it() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok(OUTPUT)
    {
        eprintln!("skipped: not inside the Hyprland harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let daemon = start_daemon(root);
    let log = || std::fs::read_to_string(root.join("daemon.log")).unwrap_or_default();
    let ready = until(30, || {
        gdbus(
            &format!("{}.SetPointer", es_protocol::TEST_INTERFACE),
            &["20100", "100"],
        )
    });
    assert!(ready.is_some(), "daemon never answered:\n{}", log());
    sleep(Duration::from_millis(500));
    let empty = screen(root, "empty");

    assert!(command("capture-bar").is_some(), "{}", log());
    sleep(Duration::from_millis(1500));
    let bar = screen(root, "bar");
    let covered = bar_pixels(&empty, &bar, &bar);
    assert!(covered > 20_000, "no bar on screen ({covered}):\n{}", log());

    // The same path as Area on the bar: the capture closes the bar, waits, then freezes the screen.
    assert!(command("capture/region").is_some(), "{}", log());
    sleep(Duration::from_millis(1500));
    let frozen = screen(root, "frozen");
    let left = bar_pixels(&empty, &bar, &frozen);
    gdbus(
        &format!("{}.Inject", es_protocol::TEST_INTERFACE),
        &[r#"[{"type":"key","keystroke":"escape"}]"#],
    );
    assert!(
        left < covered / 100,
        "{left} of the bar's {covered} pixels are in the frozen frame:\n{}",
        log()
    );
    drop(daemon);
}
