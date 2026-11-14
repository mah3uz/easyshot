#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use serde_json::Value;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const OUTPUT: &str = "easyshot-test";
const HIDDEN: &str = "7312";
const TEST_OUTPUT_X: i64 = 20000;

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

fn hyprctl(args: &[&str]) -> String {
    let out = Command::new("hyprctl").args(args).output().unwrap();
    assert!(out.status.success(), "hyprctl {args:?}");
    String::from_utf8(out.stdout).unwrap()
}

fn hypr_json(what: &str) -> Value {
    serde_json::from_str(&hyprctl(&[what, "-j"])).unwrap()
}

fn dispatch(call: &str) {
    assert_eq!(hyprctl(&["dispatch", call]).trim(), "ok", "{call}");
}

fn json_line(stdout: &str) -> Value {
    assert_eq!(stdout.lines().count(), 1, "one JSON object: {stdout:?}");
    serde_json::from_str(stdout).unwrap()
}

fn agent(root: &Path, args: &[&str]) -> (i32, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("agent").args(args).arg("--json");
    xdg(&mut cmd, root);
    let out = cmd.output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    (out.status.code().unwrap_or(-1), json_line(&text))
}

/// What a capture of a hidden window must not disturb.
#[derive(Debug)]
struct Desktop {
    workspaces: Vec<(String, i64)>,
    focused: String,
    cursor: (i64, i64),
}

// The maintainer's own pointer moves during a run, so it is only checked to stay off the test output, where focusing a window would put it.
impl PartialEq for Desktop {
    fn eq(&self, other: &Desktop) -> bool {
        self.workspaces == other.workspaces
            && self.focused == other.focused
            && self.cursor.0 < TEST_OUTPUT_X
            && other.cursor.0 < TEST_OUTPUT_X
    }
}

fn desktop() -> Desktop {
    let monitors = hypr_json("monitors");
    Desktop {
        workspaces: monitors
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m["name"].as_str().unwrap().to_owned(),
                    m["activeWorkspace"]["id"].as_i64().unwrap(),
                )
            })
            .collect(),
        focused: hypr_json("activewindow")["address"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        cursor: {
            let pos = hyprctl(&["cursorpos"]);
            let (x, y) = pos.trim().split_once(',').unwrap();
            (x.trim().parse().unwrap(), y.trim().parse().unwrap())
        },
    }
}

fn client(class: &str) -> Option<Value> {
    hypr_json("clients")
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["class"] == class)
        .cloned()
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

struct Window {
    class: String,
    address: String,
}

impl Drop for Window {
    fn drop(&mut self) {
        let _ = Command::new("hyprctl")
            .arg("dispatch")
            .arg(format!(
                "hl.dsp.window.close({{ window = \"address:{}\" }})",
                self.address
            ))
            .output();
    }
}

/// A kitty window of one flat colour, opened on `workspace` without taking focus.
fn open(class: &str, colour: &str, workspace: &str, extra: &str) -> Window {
    let cmd = format!(
        "kitty --class {class} {extra} -o background={colour} -o foreground={colour} -o cursor={colour} \
         -o cursor_blink_interval=0 -o window_padding_width=0 -o confirm_os_window_close=0 sleep 600"
    );
    dispatch(&format!(
        "hl.dsp.exec_cmd(\"{cmd}\", {{ workspace = \"{workspace} silent\" }})"
    ));
    let found = until(20, || client(class).filter(|c| c["mapped"] == true))
        .unwrap_or_else(|| panic!("no {class} window"));
    Window {
        class: class.to_owned(),
        address: found["address"].as_str().unwrap().to_owned(),
    }
}

fn move_to(window: &Window, workspace: &str) {
    dispatch(&format!(
        "hl.dsp.window.move({{ workspace = \"{workspace}\", follow = false, window = \"address:{}\" }})",
        window.address
    ));
    until(10, || {
        (client(&window.class)?["workspace"]["name"] == workspace).then_some(())
    })
    .expect("window did not move");
}

fn listed(root: &Path, class: &str) -> Option<Value> {
    let (_, reply) = agent(root, &["list", "windows"]);
    reply["windows"]
        .as_array()?
        .iter()
        .find(|w| w["app_id"] == class)
        .cloned()
}

/// Pixels of one primary colour; the session may dim windows that have no focus, so not exact.
fn share_of(path: &str, [r, g, b]: [u8; 3]) -> (f64, (u32, u32)) {
    let image = image::open(path).unwrap().to_rgba8();
    let near = image
        .pixels()
        .filter(|p| {
            [r, g, b]
                .iter()
                .zip(p.0)
                .all(|(want, got)| if *want > 0 { got >= 120 } else { got <= 20 })
        })
        .count();
    (
        near as f64 / f64::from(image.width() * image.height()),
        image.dimensions(),
    )
}

fn frames(path: &str) -> u64 {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-count_frames", "-select_streams", "v:0"])
        .args([
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
            path,
        ])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn shown_workspace() -> String {
    let monitors = hypr_json("monitors");
    let monitor = monitors
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == OUTPUT)
        .unwrap();
    monitor["activeWorkspace"]["name"]
        .as_str()
        .unwrap()
        .to_owned()
}

struct Scene {
    root: tempfile::TempDir,
    shown: String,
    before: Desktop,
    id: String,
    _red: Window,
    _daemon: Daemon,
}

impl Scene {
    fn path(&self) -> &Path {
        self.root.path()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path().join("daemon.log")).unwrap_or_default()
    }

    fn unchanged(&self, after: &str) {
        assert_eq!(desktop(), self.before, "{after} disturbed the desktop");
    }
}

// The scenes run inside tests/harness/hypr.sh, whose only output is easyshot-test; skipped anywhere else.
fn scene() -> Option<Scene> {
    if std::env::var("EASYSHOT_TEST").as_deref() != Ok("1")
        || std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() != Ok(OUTPUT)
    {
        eprintln!("skipped: not inside the Hyprland harness");
        return None;
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("config/easyshot")).unwrap();
    std::fs::write(
        root.path().join("config/easyshot/config.toml"),
        "[agent]\nenabled = true\nconsent = \"always-allow\"\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.arg("daemon").env("EASYSHOT_LOG", "info,easyshot=debug");
    xdg(&mut cmd, root.path());
    let log = std::fs::File::create(root.path().join("daemon.log")).unwrap();
    let daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());
    until(30, || {
        (agent(root.path(), &["list", "outputs"]).0 != 3).then_some(())
    })
    .expect("daemon never answered");

    hyprctl(&[
        "eval",
        &format!("hl.workspace_rule({{ workspace = \"{HIDDEN}\", monitor = \"{OUTPUT}\" }})"),
    ]);
    let shown = shown_workspace();
    let before = desktop();
    let red = open("esh-red", "#ff0000", &shown, "");
    move_to(&red, HIDDEN);
    assert_eq!(shown_workspace(), shown);
    // A window that was just moved is still resizing; a still capture takes one frame and does not retry.
    sleep(Duration::from_secs(2));
    let entry = until(30, || listed(root.path(), "esh-red")).expect("hidden window never listed");
    assert_eq!(entry["visible"], Value::Bool(false), "{entry}");
    let id = entry["id"].as_str().unwrap().to_owned();
    let scene = Scene {
        root,
        shown,
        before,
        id,
        _red: red,
        _daemon: daemon,
    };
    scene.unchanged("moving and listing");
    Some(scene)
}

#[test]
fn platform_hypr_agent_captures_and_reads_a_window_on_a_hidden_workspace_without_moving_anything() {
    let Some(scene) = scene() else { return };
    let root = scene.path();
    let size = client("esh-red").unwrap()["size"].clone();
    let (w, h) = (
        size[0].as_f64().unwrap() * 1.5,
        size[1].as_f64().unwrap() * 1.5,
    );
    let (code, reply) = agent(root, &["capture", "--window", &scene.id]);
    assert_eq!(code, 0, "{reply}\n{}", scene.log());
    let (red_share, (pw, ph)) = share_of(reply["path"].as_str().unwrap(), [255, 0, 0]);
    assert!(
        red_share > 0.9,
        "{red_share} of the capture is red: {reply}"
    );
    assert!(
        (f64::from(pw) - w).abs() <= 2.0 && (f64::from(ph) - h).abs() <= 2.0,
        "{pw}x{ph} against {w}x{h}"
    );
    assert_eq!(reply["window"]["id"].as_str(), Some(scene.id.as_str()));
    scene.unchanged("capturing");

    let (code, reply) = agent(root, &["ocr", "--window", &scene.id]);
    assert_eq!(code, 0, "{reply}\n{}", scene.log());
    scene.unchanged("reading text");

    let _green = open("esh-green", "#00ff00", &scene.shown, "");
    let cover = open(
        "esh-cover",
        "#0000ff",
        &scene.shown,
        "--start-as fullscreen",
    );
    until(10, || {
        (client(&cover.class)?["fullscreen"].as_i64()? > 0).then_some(())
    })
    .expect("the cover window is not fullscreen");
    scene.unchanged("opening the windows");
    sleep(Duration::from_secs(2));
    let covered = until(30, || listed(root, "esh-green")).unwrap();
    let id = covered["id"].as_str().unwrap();
    let (code, reply) = agent(root, &["capture", "--window", id]);
    assert_eq!(code, 0, "{reply}\n{}", scene.log());
    let path = reply["path"].as_str().unwrap();
    let (green_share, _) = share_of(path, [0, 255, 0]);
    assert!(
        green_share > 0.9,
        "{green_share} of the capture is green: {reply}"
    );
    let (blue_share, _) = share_of(path, [0, 0, 255]);
    assert!(
        blue_share < 0.01,
        "the covering window shows in the capture"
    );
    scene.unchanged("capturing a covered window");
}

#[test]
#[ignore = "test mode installs no window recorder, so a hidden window cannot be recorded in the harness"]
fn platform_hypr_agent_records_a_window_on_a_hidden_workspace_without_moving_anything() {
    let Some(scene) = scene() else { return };
    let root = scene.path();
    let (code, started) = agent(
        root,
        &[
            "record",
            "start",
            "--window",
            &scene.id,
            "--max-seconds",
            "2",
        ],
    );
    assert_eq!(code, 0, "{started}\n{}", scene.log());
    scene.unchanged("starting a recording");
    sleep(Duration::from_millis(1500));
    let (code, stopped) = agent(root, &["record", "stop"]);
    assert_eq!(code, 0, "{stopped}\n{}", scene.log());
    let video = stopped["path"].as_str().unwrap();
    assert!(Path::new(video).is_file(), "no video at {video}: {stopped}");
    assert!(frames(video) > 0, "no frames in {video}");
    scene.unchanged("stopping a recording");
}
