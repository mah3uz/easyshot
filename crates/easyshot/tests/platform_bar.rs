#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEST: &str = es_protocol::BUS_NAME_TEST;
const PATH: &str = es_protocol::OBJECT_PATH;

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

fn screen(root: &Path) -> image::RgbaImage {
    let file = root.join("screen.png");
    let status = Command::new("grim")
        .args(["-o", "HEADLESS-1"])
        .arg(&file)
        .status()
        .unwrap();
    assert!(status.success(), "grim");
    image::open(&file).unwrap().to_rgba8()
}

fn changed(a: &image::RgbaImage, b: &image::RgbaImage) -> usize {
    a.pixels().zip(b.pixels()).filter(|(x, y)| x != y).count()
}

// The bar is 676x64 logical at scale 1.5: tens of thousands of pixels when it shows.
const SHOWN: usize = 20_000;

fn bar_shown(root: &Path, empty: &image::RgbaImage, want: bool) -> bool {
    until(5, || {
        ((changed(empty, &screen(root)) > SHOWN) == want).then_some(())
    })
    .is_some()
}

// Runs inside tests/harness/sway.sh, whose only output is HEADLESS-1; skipped anywhere else.
#[test]
fn platform_bar_takes_the_keyboard_and_every_open_toggles_it() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let daemon = start_daemon(root);
    let log = || std::fs::read_to_string(root.join("daemon.log")).unwrap_or_default();
    let ready = until(30, || {
        gdbus(
            &format!("{}.SetPointer", es_protocol::TEST_INTERFACE),
            &["10", "10"],
        )
    });
    assert!(ready.is_some(), "daemon never answered:\n{}", log());
    sleep(Duration::from_millis(500));
    let empty = screen(root);

    assert!(command("capture-bar").is_some(), "{}", log());
    assert!(bar_shown(root, &empty, true), "no bar:\n{}", log());
    // The harness seat has no keyboard until wtype adds one; a press sent as it is torn down never arrives.
    let esc = Command::new("wtype")
        .args(["-s", "300", "-k", "Escape", "-s", "300"])
        .status()
        .unwrap();
    assert!(esc.success(), "wtype");
    assert!(
        bar_shown(root, &empty, false),
        "Esc from the compositor's keyboard did not reach the bar:\n{}",
        log()
    );

    assert!(command("capture-bar").is_some(), "{}", log());
    assert!(
        bar_shown(root, &empty, true),
        "bar did not reopen:\n{}",
        log()
    );
    assert!(command("capture-bar").is_some(), "{}", log());
    assert!(
        bar_shown(root, &empty, false),
        "a second open left the bar up:\n{}",
        log()
    );
    assert!(command("capture-bar").is_some(), "{}", log());
    assert!(bar_shown(root, &empty, true), "a third open:\n{}", log());
    drop(daemon);
}
