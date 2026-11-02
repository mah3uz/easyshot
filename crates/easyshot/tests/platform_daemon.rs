#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use es_backend::{Backend, Probe};
use es_geom::Point;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEST: &str = "dev.easyshot.EasyshotTest";
const PATH: &str = "/dev/easyshot/Easyshot";

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

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect()
}

fn staged_cards(deck: &Path) -> usize {
    std::fs::read_dir(deck).map_or(0, |d| {
        d.filter_map(Result::ok)
            .filter(|e| e.path().join("item.json").exists())
            .count()
    })
}

struct Daemon(Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = gdbus("dev.easyshot.Easyshot.Command", &["quit", "{}"]);
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
            "[general]\nsave_dir = \"{}\"\n\n[after_capture.screenshot]\nsave = true\nshow_preview = true\n",
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

fn output_origin() -> Point {
    let outputs: serde_json::Value =
        serde_json::from_str(&swaymsg(&["-t", "get_outputs", "-r"])).unwrap();
    let rect = &outputs[0]["rect"];
    Point::new(rect["x"].as_f64().unwrap(), rect["y"].as_f64().unwrap())
}

// Runs inside tests/harness/sway.sh, whose only output is HEADLESS-1; skipped anywhere else.
#[test]
fn platform_daemon_region_capture_reaches_the_deck_and_the_save_folder() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let daemon = start_daemon(root.path());
    let origin = output_origin();
    let ready = until(30, || {
        gdbus(
            "dev.easyshot.Test.SetPointer",
            &[
                &(origin.x + 200.).to_string(),
                &(origin.y + 150.).to_string(),
            ],
        )
    });
    let log = || std::fs::read_to_string(root.path().join("daemon.log")).unwrap_or_default();
    assert!(ready.is_some(), "daemon never answered:\n{}", log());

    let probe = Probe::gather().unwrap();
    let backend = Backend::detect(&probe);
    let input = backend.virtual_input.as_ref().expect("virtual pointer");
    let to = |x: f64, y: f64| {
        input
            .scroll(Point::new(origin.x + x, origin.y + y), 0.0)
            .unwrap();
        sleep(Duration::from_millis(40));
    };
    to(200., 150.);

    let job = gdbus("dev.easyshot.Easyshot.Command", &["capture/region", "{}"]);
    assert!(job.is_some(), "Command refused:\n{}", log());
    sleep(Duration::from_millis(2500));
    to(200., 150.);
    swaymsg(&["seat", "seat0", "cursor", "press", "button1"]);
    for step in 1..=10 {
        let t = f64::from(step) / 10.;
        to(200. + 304. * t, 150. + 151. * t);
    }
    swaymsg(&["seat", "seat0", "cursor", "release", "button1"]);
    sleep(Duration::from_millis(300));
    to(352., 225.);
    for _ in 0..2 {
        swaymsg(&["seat", "seat0", "cursor", "press", "button1"]);
        swaymsg(&["seat", "seat0", "cursor", "release", "button1"]);
    }

    let saved = until(15, || {
        let found = files(&root.path().join("saved"), "png");
        (!found.is_empty()).then_some(found)
    });
    let deck = root.path().join("data/easyshot/deck");
    let shot = std::env::var_os("EASYSHOT_E2E_SHOT").map(PathBuf::from);
    if let Some(shot) = &shot {
        sleep(Duration::from_millis(500));
        let _ = Command::new("grim")
            .args(["-o", "HEADLESS-1"])
            .arg(shot)
            .status();
    }
    let saved = saved.unwrap_or_else(|| panic!("nothing saved:\n{}", log()));
    assert_eq!(saved.len(), 1);
    let (w, h) = image::image_dimensions(&saved[0]).unwrap();
    eprintln!("SAVED {} {w}x{h}", saved[0].display());
    assert!(
        (455..=457).contains(&w) && (225..=228).contains(&h),
        "304x151 at scale 1.5, got {w}x{h}"
    );
    assert_eq!(staged_cards(&deck), 1, "one card staged:\n{}", log());
    drop(daemon);
}
