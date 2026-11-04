#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEST: &str = es_protocol::BUS_NAME_TEST;
const PATH: &str = es_protocol::OBJECT_PATH;
const OUTPUT: &str = "easyshot-test";
const TITLE: &str = "easyshot-scroll-page";

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

fn inject(events: &str) {
    gdbus(
        &format!("{}.Inject", es_protocol::TEST_INTERFACE),
        &[events],
    )
    .unwrap();
}

// The compositor's control tool is reached through the harness, which owns every compositor-specific call.
fn compositor_ctl(args: &[&str]) -> String {
    let harness = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/harness/hypr.sh");
    let out = Command::new("bash")
        .arg(harness)
        .arg("ctl")
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "harness ctl {args:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn monitor() -> serde_json::Value {
    let all: serde_json::Value =
        serde_json::from_str(&compositor_ctl(&["monitors", "-j"])).unwrap();
    all.as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == OUTPUT)
        .cloned()
        .unwrap()
}

fn page_pid() -> Option<i64> {
    let clients: serde_json::Value =
        serde_json::from_str(&compositor_ctl(&["clients", "-j"])).unwrap();
    clients
        .as_array()?
        .iter()
        .find(|c| c["title"] == TITLE)
        .and_then(|c| c["pid"].as_i64())
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

fn pngs(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect()
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

struct Page;

impl Drop for Page {
    fn drop(&mut self) {
        if let Some(pid) = page_pid() {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
    }
}

// The page scrolls itself under an animation fixed on screen, so nothing sends input on the user's session.
fn open_page(root: &Path) -> Page {
    let page = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/scroll_page.html");
    let ws = monitor()["activeWorkspace"]["id"].as_i64().unwrap();
    let exec = format!(
        "google-chrome-stable --user-data-dir={} --no-first-run --no-default-browser-check \
         --ozone-platform=wayland '--app=file://{page}?step=60&every=300&anim=1&wait=4000'",
        root.join("chrome").display()
    );
    compositor_ctl(&[
        "dispatch",
        &format!("hl.dsp.exec_cmd(\"{exec}\", {{ workspace = \"{ws} silent\" }})"),
    ]);
    let opened = Page;
    assert!(until(20, page_pid).is_some(), "the page never opened");
    sleep(Duration::from_millis(1500));
    opened
}

fn start_daemon(root: &Path) -> Daemon {
    let config = root.join("config/easyshot");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.toml"),
        format!(
            "[general]\nsave_dir = \"{}\"\n\n[after_capture.screenshot]\nshow_preview = true\n",
            root.join("saved").display()
        ),
    )
    .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_easyshot"))
        .arg("daemon")
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env("EASYSHOT_TEST_DIR", root.join("test"))
        .env_remove("EASYSHOT_TEST_VIRTUAL_INPUT")
        .env(
            "EASYSHOT_LOG",
            std::env::var("EASYSHOT_E2E_LOG").unwrap_or_else(|_| "info".into()),
        )
        .stderr(Stdio::from(
            std::fs::File::create(root.join("daemon.log")).unwrap(),
        ))
        .spawn()
        .unwrap();
    Daemon(child)
}

// Runs inside tests/harness/hypr.sh, whose only output is easyshot-test; skipped anywhere else.
#[test]
fn platform_hypr_scroll_capture_follows_a_page_that_scrolls_under_an_animation() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok(OUTPUT)
    {
        eprintln!("skipped: not inside the Hyprland harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let page = open_page(root.path());
    let daemon = start_daemon(root.path());
    let log = || std::fs::read_to_string(root.path().join("daemon.log")).unwrap_or_default();
    let ready = until(30, || {
        gdbus(
            &format!("{}.SetPointer", es_protocol::TEST_INTERFACE),
            &["20100", "100"],
        )
    });
    assert!(ready.is_some(), "daemon never answered:\n{}", log());

    assert!(command("capture/scroll").is_some(), "refused:\n{}", log());
    sleep(Duration::from_millis(2500));
    inject(r#"[{"type":"key","keystroke":"space"}]"#);
    sleep(Duration::from_millis(500));
    inject(
        r#"[{"type":"mouse_move","x":100,"y":100},{"type":"mouse_down","x":100,"y":100},
            {"type":"mouse_move","x":300,"y":250},{"type":"mouse_move","x":700,"y":500},
            {"type":"mouse_up","x":700,"y":500},{"type":"key","keystroke":"enter"}]"#,
    );
    sleep(Duration::from_millis(4000));
    if let Some(shot) = std::env::var_os("EASYSHOT_E2E_SHOT") {
        let _ = Command::new("grim").args(["-o", OUTPUT]).arg(shot).status();
    }
    assert!(command("capture/scroll").is_some(), "refused:\n{}", log());
    let saved = until(15, || {
        let found = pngs(&root.path().join("saved"));
        (!found.is_empty()).then_some(found)
    });
    drop(page);
    let saved = saved.unwrap_or_else(|| panic!("nothing saved:\n{}", log()));
    let (w, h) = image::image_dimensions(&saved[0]).unwrap();
    eprintln!("SAVED {} {w}x{h}", saved[0].display());
    assert!((899..=901).contains(&w), "600 points at scale 1.5, got {w}");
    assert!(
        h >= 2 * 600,
        "the stitched page should be taller than two areas, got {h}"
    );
    drop(daemon);
}
