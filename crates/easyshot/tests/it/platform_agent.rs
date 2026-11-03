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

fn allow_agents(root: &Path) {
    std::fs::create_dir_all(root.join("config/easyshot")).unwrap();
    std::fs::write(
        root.join("config/easyshot/config.toml"),
        "[agent]\nenabled = true\nconsent = \"always-allow\"\n",
    )
    .unwrap();
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
    allow_agents(root.path());
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

fn capture(root: &Path, args: &[&str]) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.args(["agent", "capture", "--json"]).args(args);
    xdg(&mut cmd, root);
    let out = cmd.output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    (out.status.code().unwrap_or(-1), json_line(&text))
}

fn dirs_named(root: &Path, name: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == name) {
                    found.push(path.clone());
                }
                todo.push(path);
            }
        }
    }
    found
}

fn entries(root: &Path, name: &str) -> Vec<std::path::PathBuf> {
    dirs_named(&root.join("data"), name)
        .iter()
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .map(|e| e.path())
        .collect()
}

fn library_images(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = entries(root, "library");
    found.retain(|p| p.extension().is_some_and(|e| e == "png"));
    found
}

fn within(a: u32, b: f64) -> bool {
    (f64::from(a) - b).abs() <= 2.0
}

#[test]
fn platform_agent_captures_by_geometry_output_window_and_fullscreen() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    std::fs::create_dir_all(root.join("config/easyshot")).unwrap();
    std::fs::write(
        root.join("config/easyshot/config.toml"),
        "[capture]\nkeep_in_deck_until_saved = true\n\n[agent]\nenabled = true\nconsent = \"always-allow\"\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("daemon");
    xdg(&mut cmd, root);
    let log = std::fs::File::create(root.join("daemon.log")).unwrap();
    let _daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(30);
    while agent(root, "outputs").0 == 3 && Instant::now() < end {
        sleep(Duration::from_millis(200));
    }

    let size = |reply: &Value| {
        let path = reply["path"].as_str().unwrap();
        let (w, h) = image::image_dimensions(path).unwrap();
        assert_eq!(
            (reply["width"].as_u64(), reply["height"].as_u64()),
            (Some(w.into()), Some(h.into()))
        );
        (w, h)
    };

    let (code, reply) = capture(root, &["--geometry", "100,50 320x200", "--no-ui"]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(
        (reply["kind"].as_str(), reply["output"].as_str()),
        (Some("capture"), Some("HEADLESS-1"))
    );
    assert_eq!(reply["scale"], 1.5);
    assert_eq!(size(&reply), (480, 300), "logical 320x200 at scale 1.5");
    assert!(reply.get("window").is_none());
    assert_eq!(library_images(root).len(), 1, "{:?}", library_images(root));
    assert!(entries(root, "deck").is_empty(), "no deck card for --no-ui");

    let (code, reply) = capture(root, &["--output", "HEADLESS-1", "--no-ui"]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(size(&reply), (1920, 1080));
    let (code, reply) = capture(root, &["--fullscreen", "--no-ui"]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(size(&reply), (1920, 1080));
    assert!(entries(root, "deck").is_empty());

    let (code, reply) = capture(root, &["--fullscreen"]);
    assert_eq!(code, 0, "{reply}");
    assert!(
        !entries(root, "deck").is_empty(),
        "a capture with the interface stages a card"
    );

    std::fs::create_dir_all(root.join("exports")).unwrap();
    let out = root.join("exports/shot.png");
    let library = library_images(root).len();
    let (code, reply) = capture(
        root,
        &[
            "--geometry",
            "0,0 64x48",
            "--no-ui",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{reply}");
    assert_eq!(reply["path"].as_str(), out.to_str());
    assert_eq!(image::image_dimensions(&out).unwrap(), (96, 72));
    assert_eq!(
        library_images(root).len(),
        library,
        "--out with --no-ui writes only the file"
    );

    let started = Instant::now();
    let (code, _) = capture(root, &["--fullscreen", "--no-ui", "--delay", "1"]);
    assert_eq!(code, 0);
    assert!(started.elapsed() >= Duration::from_secs(1));

    let (code, reply) = capture(root, &["--output", "NOPE-9", "--no-ui"]);
    assert_eq!(
        (code, reply["error"]["code"].as_str()),
        (5, Some("not_found")),
        "{reply}"
    );
    let (code, reply) = capture(root, &["--geometry", "5000,5000 10x10", "--no-ui"]);
    assert_eq!(
        (code, reply["error"]["code"].as_str()),
        (5, Some("not_found")),
        "{reply}"
    );
    let (code, reply) = capture(root, &["--window", "no such window", "--no-ui"]);
    assert_eq!(
        (code, reply["error"]["code"].as_str()),
        (5, Some("not_found")),
        "{reply}"
    );

    let library_png = library_images(root).remove(0);
    for args in [
        vec!["gallery"],
        vec!["open", out.to_str().unwrap()],
        vec!["open", library_png.to_str().unwrap()],
    ] {
        let mut open = Command::new(env!("CARGO_BIN_EXE_easyshot"));
        open.args(args);
        xdg(&mut open, root);
        assert!(open.status().unwrap().success());
    }
    let end = Instant::now() + Duration::from_secs(30);
    let windows = loop {
        let (_, text) = agent(root, "windows");
        let windows = json_line(&text)["windows"].as_array().unwrap().clone();
        if windows.len() >= 3 || Instant::now() > end {
            break windows;
        }
        sleep(Duration::from_millis(200));
    };
    assert_eq!(windows.len(), 3, "{windows:?}");
    sleep(Duration::from_millis(500));
    let (_, text) = agent(root, "windows");
    let windows = json_line(&text)["windows"].as_array().unwrap().clone();
    let target = &windows[0];
    let id = target["id"].as_str().unwrap();
    let (code, reply) = capture(root, &["--window", id, "--no-ui"]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(reply["window"]["id"].as_str(), Some(id));
    let (w, h) = size(&reply);
    assert!(
        within(w, target["width"].as_f64().unwrap() * 1.5)
            && within(h, target["height"].as_f64().unwrap() * 1.5),
        "{w}x{h} against {target}"
    );

    let shared = "dev.easyshot.app.editor";
    let editors: Vec<&Value> = windows
        .iter()
        .filter(|w| w["app_id"].as_str() == Some(shared))
        .collect();
    assert_eq!(editors.len(), 2, "two editor windows: {windows:?}");
    let (code, reply) = capture(root, &["--window", shared, "--no-ui"]);
    assert_eq!(
        (code, reply["error"]["code"].as_str()),
        (4, Some("invalid_arguments")),
        "{reply}"
    );
    let message = reply["error"]["message"].as_str().unwrap();
    for w in editors {
        assert!(message.contains(w["id"].as_str().unwrap()), "{message}");
    }
}

#[test]
fn platform_agent_reads_text_from_a_geometry_target_and_a_file() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    allow_agents(root);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("daemon")
        .env("EASYSHOT_TEST_OCR_TEXT", "fixed words");
    xdg(&mut cmd, root);
    let log = std::fs::File::create(root.join("daemon.log")).unwrap();
    let _daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(30);
    while agent(root, "outputs").0 == 3 && Instant::now() < end {
        sleep(Duration::from_millis(200));
    }
    let ocr = |args: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
        cmd.args(["agent", "ocr", "--json"]).args(args);
        xdg(&mut cmd, root);
        let out = cmd.output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        (out.status.code().unwrap_or(-1), json_line(&text))
    };
    let (code, reply) = ocr(&["--geometry", "100,50 320x200"]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(reply["kind"], "ocr");
    assert_eq!(reply["text"], "fixed words");
    assert_eq!(reply["source"]["output"], "HEADLESS-1");
    assert_eq!(
        (
            reply["lines"][0]["width"].as_u64(),
            reply["lines"][0]["height"].as_u64()
        ),
        (Some(480), Some(300)),
        "the line spans the 320x200 area at scale 1.5: {reply}"
    );
    assert!(library_images(root).is_empty(), "reading saves no capture");

    let png = root.join("note.png");
    image::RgbaImage::from_pixel(40, 20, image::Rgba([255; 4]))
        .save(&png)
        .unwrap();
    let (code, reply) = ocr(&["--file", png.to_str().unwrap()]);
    assert_eq!(code, 0, "{reply}");
    assert_eq!(reply["source"]["file"].as_str(), png.to_str());
    assert_eq!(reply["lines"][0]["width"], 40);
    let missing = root.join("none.png");
    let (code, reply) = ocr(&["--file", missing.to_str().unwrap()]);
    assert_eq!(
        (code, reply["error"]["code"].as_str()),
        (5, Some("not_found"))
    );
}

#[test]
fn platform_agent_records_a_geometry_target_and_stop_leaves_the_video() {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok("HEADLESS-1")
    {
        eprintln!("skipped: not inside the headless sway harness");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    std::fs::create_dir_all(root.join("config/easyshot")).unwrap();
    std::fs::write(
        root.join("config/easyshot/config.toml"),
        "[agent]\nenabled = true\nconsent = \"always-allow\"\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("daemon");
    xdg(&mut cmd, root);
    let log = std::fs::File::create(root.join("daemon.log")).unwrap();
    let _daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(30);
    while agent(root, "outputs").0 == 3 && Instant::now() < end {
        sleep(Duration::from_millis(200));
    }
    let record = |args: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
        cmd.args(["agent", "record"]).args(args).arg("--json");
        xdg(&mut cmd, root);
        let out = cmd.output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        (out.status.code().unwrap_or(-1), json_line(&text))
    };
    let (code, started) = record(&["start", "--geometry", "100,50 320x200"]);
    assert_eq!(code, 0, "{started}");
    assert_eq!(started["ok"], Value::Bool(true));
    assert_eq!(started["kind"], "recording");
    assert_eq!(started["state"], "recording", "{started}");
    let id = started["id"].as_str().unwrap().to_owned();
    assert!(!id.is_empty());

    sleep(Duration::from_millis(500));
    let (code, status) = record(&["status"]);
    assert_eq!(code, 0, "{status}");
    assert_eq!(status["id"].as_str(), Some(id.as_str()));

    let (code, stopped) = record(&["stop", "--id", &id]);
    assert_eq!(code, 0, "{stopped}");
    assert_eq!(stopped["kind"], "recording");
    let path = stopped["path"].as_str().unwrap();
    assert!(Path::new(path).is_file(), "no video at {path}: {stopped}");
    assert!(stopped["bytes"].as_u64().unwrap() > 0, "{stopped}");
}
