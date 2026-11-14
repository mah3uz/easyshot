#![cfg(all(feature = "test-hooks", feature = "gui"))]
#![allow(clippy::unwrap_used)]
use super::platform_agent::{
    Daemon, agent, allow_agents, capture, json_line, swaymsg, within, xdg,
};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

fn focused_workspace() -> String {
    let workspaces = swaymsg(&["-t", "get_workspaces", "-r"]);
    let focused = workspaces
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["focused"] == true);
    focused.unwrap()["name"].as_str().unwrap().to_owned()
}

fn listed(root: &Path, app_id: &str, visible: bool) -> Value {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let (_, text) = agent(root, "windows");
        let windows = json_line(&text)["windows"].as_array().unwrap().clone();
        let found = windows.into_iter().find(|w| w["app_id"] == app_id);
        match found {
            Some(w) if w["visible"] == visible => return w,
            other => assert!(Instant::now() < end, "{app_id} never {visible}: {other:?}"),
        }
        sleep(Duration::from_millis(200));
    }
}

struct Terminal(&'static str);

impl Terminal {
    fn open(app: &'static str, colour: &str) -> Terminal {
        swaymsg(&[&format!(
            "exec alacritty --class {app} -o 'colors.primary.background=\"{colour}\"' -e sleep 120"
        )]);
        Terminal(app)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = Command::new("swaymsg")
            .arg(format!("[app_id={}] kill", self.0))
            .output();
    }
}

/// The colour the screen shows at the centre of `w`, as a 10x10 crop reads it.
fn shown_colour(root: &Path, w: &Value) -> image::Rgba<u8> {
    let (x, y) = (w["x"].as_i64().unwrap(), w["y"].as_i64().unwrap());
    let (cx, cy) = (
        x + w["width"].as_i64().unwrap() / 2,
        y + w["height"].as_i64().unwrap() / 2,
    );
    let (code, reply) = capture(root, &["--geometry", &format!("{cx},{cy} 10x10")]);
    assert_eq!(code, 0, "{reply}");
    *image::open(reply["path"].as_str().unwrap())
        .unwrap()
        .to_rgba8()
        .get_pixel(5, 5)
}

fn share_of(image: &image::RgbaImage, colour: image::Rgba<u8>) -> f64 {
    let near = |p: &image::Rgba<u8>| (0..3).all(|i| p[i].abs_diff(colour[i]) <= 12);
    image.pixels().filter(|p| near(p)).count() as f64 / image.pixels().count() as f64
}

fn window_shot(root: &Path, id: &str, logical: (f64, f64)) -> image::RgbaImage {
    let (code, reply) = capture(root, &["--window", id]);
    assert_eq!(code, 0, "{reply}");
    let image = image::open(reply["path"].as_str().unwrap())
        .unwrap()
        .to_rgba8();
    let (w, h) = image.dimensions();
    assert!(
        within(w, logical.0 * 1.5) && within(h, logical.1 * 1.5),
        "{w}x{h}: a crop of {logical:?} at scale 1.5 has the window's size"
    );
    image
}

// An agent asks for a window while the user works: it gets that window's own pixels, at the size
// a crop gives, whether the window is shown, covered or on another workspace, and nothing moves.
#[test]
fn platform_agent_captures_a_shown_covered_and_hidden_window_as_itself() {
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
    cmd.arg("daemon");
    xdg(&mut cmd, root);
    let log = std::fs::File::create(root.join("daemon.log")).unwrap();
    let _daemon = Daemon(cmd.stderr(Stdio::from(log)).spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(30);
    while agent(root, "outputs").0 == 3 && Instant::now() < end {
        sleep(Duration::from_millis(200));
    }

    let (app, cover) = ("es-own-green", "es-cover-red");
    let _green = Terminal::open(app, "#20c040");
    let shown = listed(root, app, true);
    let id = shown["id"].as_str().unwrap().to_owned();
    let logical = (
        shown["width"].as_f64().unwrap(),
        shown["height"].as_f64().unwrap(),
    );
    let green = shown_colour(root, &shown);
    let image = window_shot(root, &id, logical);
    assert!(share_of(&image, green) > 0.9, "the shown window: {green:?}");

    let covering = Terminal::open(cover, "#c02020");
    listed(root, cover, true);
    swaymsg(&[&format!(
        "[app_id={cover}] floating enable, resize set 1280 720, move position 0 0"
    )]);
    sleep(Duration::from_millis(500));
    let red = shown_colour(root, &shown);
    assert!(
        red[0] > red[1],
        "the screen shows the cover over the window: {red:?}"
    );
    let image = window_shot(root, &id, logical);
    drop(covering);
    assert!(
        share_of(&image, green) > 0.9,
        "the covered window, not its cover"
    );

    swaymsg(&[&format!("[app_id={app}] move container to workspace 7")]);
    let before = focused_workspace();
    listed(root, app, false);
    let image = window_shot(root, &id, logical);
    assert_eq!(
        focused_workspace(),
        before,
        "the agent switched the workspace"
    );
    assert!(
        share_of(&image, green) > 0.9,
        "the hidden window, not the screen"
    );
}
