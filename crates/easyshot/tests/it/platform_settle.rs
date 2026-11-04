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

// The compositor's control tool is reached through the harness, which owns every compositor-specific call.
fn compositor_ctl(what: &str) -> serde_json::Value {
    let harness = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/harness/hypr.sh");
    let out = Command::new("bash")
        .arg(harness)
        .args(["ctl", what, "-j"])
        .output()
        .unwrap();
    assert!(out.status.success(), "harness ctl {what}");
    serde_json::from_slice(&out.stdout).unwrap()
}

/// The bar's layer surface in the output's pixels, so the session's own panels changing on the
/// output are not taken for the bar.
fn bar_rect() -> Option<[u32; 4]> {
    let monitors = compositor_ctl("monitors");
    let monitor = monitors.as_array()?.iter().find(|m| m["name"] == OUTPUT)?;
    let layers = compositor_ctl("layers");
    let surface = layers[OUTPUT]["levels"]
        .as_object()?
        .values()
        .filter_map(|level| level.as_array())
        .flatten()
        .find(|l| l["namespace"] == "easyshot-bar")?;
    let num = |v: &serde_json::Value| v.as_f64();
    let scale = num(&monitor["scale"])?;
    let px = |v: f64| (v * scale).round() as u32;
    Some([
        px(num(&surface["x"])? - num(&monitor["x"])?),
        px(num(&surface["y"])? - num(&monitor["y"])?),
        px(num(&surface["w"])?),
        px(num(&surface["h"])?),
    ])
}

/// Of the pixels the bar covered, how many differ between `before` and `now`.
fn bar_pixels(
    [x, y, w, h]: [u32; 4],
    empty: &image::RgbaImage,
    bar: &image::RgbaImage,
    before: &image::RgbaImage,
    now: &image::RgbaImage,
) -> usize {
    let at = |i: &image::RgbaImage, px, py| *i.get_pixel(px, py);
    let (right, bottom) = ((x + w).min(empty.width()), (y + h).min(empty.height()));
    (y..bottom)
        .flat_map(|py| (x..right).map(move |px| (px, py)))
        .filter(|&(px, py)| {
            differs(&at(empty, px, py), &at(bar, px, py))
                && differs(&at(before, px, py), &at(now, px, py))
        })
        .count()
}

fn escape() {
    gdbus(
        &format!("{}.Inject", es_protocol::TEST_INTERFACE),
        &[r#"[{"type":"key","keystroke":"escape"}]"#],
    );
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

    // The selector dims the frozen frame, so it is compared with the empty screen seen through it.
    assert!(command("capture/region").is_some(), "{}", log());
    sleep(Duration::from_millis(1500));
    let selector = screen(root, "selector");
    escape();
    sleep(Duration::from_millis(1000));

    assert!(command("capture-bar").is_some(), "{}", log());
    sleep(Duration::from_millis(1500));
    let bar = screen(root, "bar");
    let rect = bar_rect().unwrap_or_else(|| panic!("no easyshot-bar layer:\n{}", log()));
    eprintln!("bar at {rect:?}");
    let covered = bar_pixels(rect, &empty, &bar, &empty, &bar);
    assert!(covered > 20_000, "no bar on screen ({covered}):\n{}", log());

    // The same path as Area on the bar: the capture closes the bar, waits, then freezes the screen.
    assert!(command("capture/region").is_some(), "{}", log());
    sleep(Duration::from_millis(1500));
    let frozen = screen(root, "frozen");
    let left = bar_pixels(rect, &empty, &bar, &selector, &frozen);
    escape();
    eprintln!("{left} of the bar's {covered} pixels are in the frozen frame");
    assert!(
        left < covered / 100,
        "{left} of the bar's {covered} pixels are in the frozen frame:\n{}",
        log()
    );
    drop(daemon);
}
