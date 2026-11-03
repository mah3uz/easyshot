#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use es_backend::{Backend, Probe};
use es_geom::Point;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEST: &str = es_protocol::BUS_NAME_TEST;
const PATH: &str = es_protocol::OBJECT_PATH;
const TERMINAL: &str = "easyshot-previous";
const PLACED: (f64, f64, f64, f64) = (100., 200., 600., 400.);
const SCALE: f64 = 1.5;

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

fn swaymsg(args: &[&str]) -> String {
    let out = Command::new("swaymsg").args(args).output().unwrap();
    assert!(out.status.success(), "swaymsg {args:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
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
    let mut found: Vec<PathBuf> = read
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    found.sort();
    found
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

fn find(node: &serde_json::Value, app_id: &str) -> Option<serde_json::Value> {
    if node["app_id"].as_str() == Some(app_id) {
        return Some(node.clone());
    }
    ["nodes", "floating_nodes"]
        .iter()
        .filter_map(|k| node[k].as_array())
        .flatten()
        .find_map(|n| find(n, app_id))
}

fn window(app_id: &str) -> Option<serde_json::Value> {
    let tree: serde_json::Value =
        serde_json::from_str(&swaymsg(&["-t", "get_tree", "-r"])).unwrap();
    find(&tree, app_id)
}

/// A terminal with nothing running in it looks the same every time it is taken.
fn open_terminal() {
    assert!(
        Command::new("alacritty").arg("--version").output().is_ok(),
        "these tests need alacritty in the harness"
    );
    swaymsg(&[&format!(
        "exec alacritty --config-file /dev/null --class {TERMINAL} -e sleep 600"
    )]);
    assert!(
        until(20, || window(TERMINAL)).is_some(),
        "the terminal never opened"
    );
    let (x, y, w, h) = PLACED;
    swaymsg(&[&format!(
        "[app_id={TERMINAL}] floating enable, resize set {w} {h}, move position {x} {y}"
    )]);
    let placed = until(10, || {
        let r = &window(TERMINAL)?["rect"];
        let at = (
            r["x"].as_f64()?,
            r["y"].as_f64()?,
            r["width"].as_f64()?,
            r["height"].as_f64()?,
        );
        (at == PLACED).then_some(())
    });
    assert!(placed.is_some(), "the terminal was not placed");
    sleep(Duration::from_millis(500));
}

fn close_terminal() {
    swaymsg(&[&format!("[app_id={TERMINAL}] kill")]);
    assert!(
        until(10, || window(TERMINAL).is_none().then_some(())).is_some(),
        "the terminal stayed open"
    );
}

/// Window mode, a click in the middle of the terminal; returns the one file it saved.
fn shoot_terminal(root: &Path, log: &dyn Fn() -> String) -> PathBuf {
    let probe = Probe::gather().unwrap();
    let backend = Backend::detect(&probe);
    let input = backend.virtual_input.as_ref().expect("virtual pointer");
    let (x, y, w, h) = PLACED;
    let centre = Point::new(x + w / 2., y + h / 2.);
    input.scroll(centre, 0.0).unwrap();
    assert!(command("capture/window").is_some(), "{}", log());
    sleep(Duration::from_millis(2500));
    input
        .scroll(Point::new(centre.x + 4., centre.y), 0.0)
        .unwrap();
    sleep(Duration::from_millis(300));
    input.scroll(centre, 0.0).unwrap();
    sleep(Duration::from_millis(300));
    swaymsg(&["seat", "seat0", "cursor", "press", "button1"]);
    swaymsg(&["seat", "seat0", "cursor", "release", "button1"]);
    let saved = until(15, || {
        let found = pngs(&root.join("saved"));
        (found.len() == 1).then_some(found)
    })
    .unwrap_or_else(|| panic!("the window shot was not saved:\n{}", log()));
    let state: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("state/easyshot/state.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        state["last_target"]["window"]["app"], TERMINAL,
        "the window is kept for capture previous"
    );
    saved.into_iter().next().unwrap()
}

fn skipped() -> bool {
    let outside = std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1");
    if outside {
        eprintln!("skipped: not inside the headless sway harness");
    }
    outside
}

fn ready(log: &dyn Fn() -> String) {
    let up = until(30, || {
        gdbus(
            &format!("{}.SetPointer", es_protocol::TEST_INTERFACE),
            &["10", "10"],
        )
    });
    assert!(up.is_some(), "daemon never answered:\n{}", log());
}

// Runs inside tests/harness/sway.sh, whose only output is HEADLESS-1 at scale 1.5; skipped anywhere else.
#[test]
fn platform_previous_takes_the_same_window_again() {
    if skipped() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let daemon = start_daemon(root.path());
    let log = || std::fs::read_to_string(root.path().join("daemon.log")).unwrap_or_default();
    ready(&log);
    open_terminal();
    let first = shoot_terminal(root.path(), &log);

    // Moved, so a repeat that took the old rectangle of the screen would show the background.
    swaymsg(&[&format!("[app_id={TERMINAL}] move position 500 100")]);
    sleep(Duration::from_millis(1100));
    assert!(command("capture/previous").is_some(), "{}", log());
    let both = until(15, || {
        let found = pngs(&root.path().join("saved"));
        (found.len() == 2).then_some(found)
    })
    .unwrap_or_else(|| panic!("capture previous saved nothing:\n{}", log()));
    let second = both.iter().find(|p| **p != first).unwrap();

    let a = image::open(&first).unwrap().to_rgba8();
    let b = image::open(second).unwrap().to_rgba8();
    let (_, _, w, h) = PLACED;
    let expected = ((w * SCALE) as u32, (h * SCALE) as u32);
    assert!(
        a.width().abs_diff(expected.0) <= 2 && a.height().abs_diff(expected.1) <= 2,
        "the terminal is {expected:?} pixels, the first shot {:?}",
        a.dimensions()
    );
    assert_eq!(a.dimensions(), b.dimensions(), "the same window's size");
    let background = image::Rgba([0, 0, 0, 255]);
    let middle = a.get_pixel(a.width() / 2, a.height() / 2);
    assert_ne!(*middle, background, "the first shot is the terminal");
    let same = a.pixels().zip(b.pixels()).filter(|(p, q)| p == q).count();
    let total = (a.width() * a.height()) as usize;
    assert!(
        same * 100 >= total * 99,
        "the same window's pixels: {same} of {total} match"
    );

    close_terminal();
    drop(daemon);
}

fn lit_in_toast_area(shot: &Path) -> usize {
    let img = image::open(shot).unwrap().to_rgba8();
    let (w, _) = img.dimensions();
    let (x0, x1) = (w / 2 - 315, w / 2 + 315);
    let mut lit = 0;
    for y in 0..216 {
        for x in x0..x1 {
            if img.get_pixel(x, y).0[..3].iter().any(|c| *c > 40) {
                lit += 1;
            }
        }
    }
    lit
}

fn grim(to: &Path) {
    let ok = Command::new("grim")
        .args(["-o", "HEADLESS-1"])
        .arg(to)
        .status()
        .unwrap();
    assert!(ok.success(), "grim");
}

// The toast is a 420x132 surface at the top centre; on HEADLESS-1 that is 630x198 pixels.
#[test]
fn platform_previous_of_a_closed_window_shows_a_toast_and_saves_nothing() {
    if skipped() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let daemon = start_daemon(root.path());
    let log = || std::fs::read_to_string(root.path().join("daemon.log")).unwrap_or_default();
    ready(&log);
    open_terminal();
    shoot_terminal(root.path(), &log);
    close_terminal();
    sleep(Duration::from_millis(1100));

    let screen = root.path().join("screen.png");
    grim(&screen);
    assert_eq!(lit_in_toast_area(&screen), 0, "no toast before");
    assert!(command("capture/previous").is_some(), "{}", log());
    let toast = until(5, || {
        grim(&screen);
        (lit_in_toast_area(&screen) > 5000).then_some(())
    });
    assert!(
        toast.is_some(),
        "no toast for the closed window:\n{}",
        log()
    );
    sleep(Duration::from_millis(2000));
    assert_eq!(
        pngs(&root.path().join("saved")).len(),
        1,
        "a closed window is not captured:\n{}",
        log()
    );
    drop(daemon);
}
