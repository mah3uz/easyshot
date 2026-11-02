use es_app::platform_ui::{LayerShellSurfaces, SelectMode, Selection, SelectionFlow};
use es_backend::{Backend, Probe};
use es_geom::{Point, Rect};
use es_model::platform::WindowInfo;
use es_overlays::capture::LayerShellSelectionFlow;
use es_overlays::capture::color::hex;
use gpui::{App, QuitMode};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Odd logical edges at scale 1.5 land on half pixels, where rounding origin and size separately goes wrong.
const FROM: (i32, i32) = (101, 51);
const TO: (i32, i32) = (304, 152);
const TERMINAL_BG: &str = "#3A7BD5";
const COLOR_INSET: f64 = 40.0;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Region,
    Window,
    Color,
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("selector: {message}");
    std::process::exit(1);
}

fn sway(args: &[&str]) {
    let ok = Command::new("swaymsg")
        .args(args)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("swaymsg {args:?} failed");
    }
}

fn cursor(args: &str) {
    let mut all = vec!["seat", "seat0", "cursor"];
    all.extend(args.split(' '));
    sway(&all);
}

fn grim(output: &str, path: &Path) {
    let ok = Command::new("grim")
        .args(["-c", "-o", output])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success());
    eprintln!(
        "SHOT {} {}",
        path.display(),
        if ok { "ok" } else { "failed" }
    );
}

#[expect(
    clippy::zombie_processes,
    reason = "main kills and waits for the terminal"
)]
fn terminal(backend: &Backend) -> (Child, WindowInfo) {
    sway(&["for_window [app_id=\"Alacritty\"] floating enable"]);
    let mut child = Command::new("alacritty")
        .args([
            "-o",
            &format!("colors.primary.background=\"{TERMINAL_BG}\""),
            "-e",
            "sleep",
            "600",
        ])
        .spawn()
        .unwrap_or_else(|e| fail(format!("alacritty: {e}")));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let found = backend
            .windows
            .snapshot()
            .into_iter()
            .find(|w| w.pid == Some(child.id()) && w.logical.is_some());
        if let Some(info) = found {
            thread::sleep(Duration::from_millis(500));
            return (child, info);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            fail("alacritty did not map");
        }
        thread::sleep(Duration::from_millis(100));
    }
}

// `swaymsg seat cursor set` sends no motion while a button is held, so motion comes from the virtual pointer.
fn drive(backend: &Backend, output: &str, shots: &Path, mode: Mode, target: Option<Rect>) {
    let pause = |ms| thread::sleep(Duration::from_millis(ms));
    let to = |x: i32, y: i32| {
        if let Some(input) = backend.virtual_input.as_ref()
            && let Err(e) = input.scroll(Point::new(f64::from(x), f64::from(y)), 0.0)
        {
            eprintln!("virtual pointer: {e}");
        }
        pause(40);
    };
    let click = || {
        cursor("press button1");
        cursor("release button1");
    };
    pause(5000);
    match (mode, target) {
        (Mode::Window, Some(r)) => {
            to(r.center().x as i32, r.center().y as i32);
            pause(400);
            grim(output, &shots.join("harness-window.png"));
            click();
        }
        (Mode::Color, Some(r)) => {
            to((r.x1 - COLOR_INSET) as i32, (r.y1 - COLOR_INSET) as i32);
            pause(400);
            grim(output, &shots.join("harness-color.png"));
            click();
        }
        _ => {
            to(TO.0 + 200, TO.1 + 100);
            pause(300);
            grim(output, &shots.join("harness-dimmed.png"));
            to(FROM.0, FROM.1);
            cursor("press button1");
            for step in 1..=4 {
                to(
                    FROM.0 + (TO.0 - FROM.0) * step / 4,
                    FROM.1 + (TO.1 - FROM.1) * step / 4,
                );
            }
            cursor("release button1");
            pause(400);
            grim(output, &shots.join("harness-selection.png"));
            to(200, 100);
            for _ in 0..2 {
                click();
            }
        }
    }
}

fn expected_region() -> (u32, u32) {
    let edge = |v: i32| (f64::from(v) * 1.5).round() as u32;
    (edge(TO.0) - edge(FROM.0), edge(TO.1) - edge(FROM.1))
}

/// What the compositor shows at the point the colour picker will click, read before any overlay opens.
fn shown_before(output: &str, shots: &Path, r: Rect, scale: f64) -> String {
    let path = shots.join("harness-before.png");
    grim(output, &path);
    let image = image::open(&path)
        .unwrap_or_else(|e| fail(format!("read {}: {e}", path.display())))
        .to_rgb8();
    let (x, y) = (
        ((r.x1 - COLOR_INSET).trunc() * scale).floor() as u32,
        ((r.y1 - COLOR_INSET).trunc() * scale).floor() as u32,
    );
    let [red, green, blue] = image.get_pixel(x, y).0;
    format!("#{red:02X}{green:02X}{blue:02X}")
}

fn check(mode: Mode, selection: &Selection, target: Option<Rect>, scale: f64, shown: &str) -> bool {
    match (mode, selection) {
        (
            Mode::Region,
            Selection::Region {
                output,
                logical,
                frame,
            },
        ) => {
            eprintln!(
                "SELECTED {} logical={logical:?} px={:?} expected={:?}",
                output.0,
                frame.size_px,
                expected_region()
            );
            frame.size_px == expected_region()
        }
        (Mode::Window, Selection::Window { id, frame }) => {
            let Some(r) = target else { return false };
            let want = (
                (r.width() * scale).round() as i64,
                (r.height() * scale).round() as i64,
            );
            eprintln!(
                "WINDOW {} px={:?} expected={want:?} output={:?} logical={:?}",
                id.0, frame.size_px, frame.output, frame.logical
            );
            (i64::from(frame.size_px.0) - want.0).abs() <= 1
                && (i64::from(frame.size_px.1) - want.1).abs() <= 1
        }
        (Mode::Color, Selection::Color(c)) => {
            eprintln!("COLOR {} expected={shown} terminal={TERMINAL_BG}", hex(*c));
            hex(*c) == shown
        }
        _ => false,
    }
}

fn main() {
    let shots = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("usage: selector SCREENSHOT_DIR [region|window|color]"));
    let mode = match std::env::args().nth(2).as_deref() {
        None | Some("region") => Mode::Region,
        Some("window") => Mode::Window,
        Some("color") => Mode::Color,
        Some(other) => fail(format!("unknown mode {other}")),
    };
    let probe = Probe::gather().unwrap_or_else(|e| fail(e));
    let Some(test_output) = probe.test.as_ref().map(|t| t.output.clone()) else {
        fail("refusing to run outside test mode");
    };
    let backend = Arc::new(Backend::detect(&probe));
    let Some(info) = backend
        .capture
        .outputs()
        .into_iter()
        .find(|o| o.id == test_output)
    else {
        fail("the test output is missing");
    };
    eprintln!(
        "OUTPUT {} logical={:?} scale={}",
        info.id.0, info.logical, info.scale
    );
    // Headless sway has no pointer until a virtual one exists, and only then do clients bind theirs.
    let Some(input) = backend.virtual_input.as_ref() else {
        fail("no virtual pointer: run under tests/harness/sway.sh");
    };
    if let Err(e) = input.scroll(
        Point::new(info.logical.x0 + 10., info.logical.y0 + 10.),
        0.0,
    ) {
        fail(format!("virtual pointer: {e}"));
    }
    let mut child = None;
    let mut target = None;
    if mode != Mode::Region {
        let (c, w) = terminal(&backend);
        eprintln!("TERMINAL {} {:?} logical={:?}", w.id.0, w.app_id, w.logical);
        target = w.logical;
        child = Some(c);
    }
    let scale = info.scale;
    let shown = match (mode, target) {
        (Mode::Color, Some(r)) => shown_before(&test_output.0, &shots, r, scale),
        _ => String::new(),
    };
    thread::spawn({
        let (backend, output) = (backend.clone(), test_output.0.clone());
        move || drive(&backend, &output, &shots, mode, target)
    });
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            es_ui::hello::init(cx);
            let surfaces = Rc::new(LayerShellSurfaces::new(
                backend.clone(),
                Some(test_output.clone()),
            ));
            let flow = LayerShellSelectionFlow::new(surfaces, backend).with_last_region(
                Rc::new(|_| None),
                Rc::new(|last, _| eprintln!("REMEMBER {last:?}")),
            );
            let task = flow.run(
                match mode {
                    Mode::Region => SelectMode::Region,
                    Mode::Window => SelectMode::Window,
                    Mode::Color => SelectMode::Color,
                },
                cx,
            );
            let unsupported = flow.run(SelectMode::Scroll, cx);
            cx.spawn(async move |cx| {
                let code = match (task.await, unsupported.await) {
                    (Ok(selection), Err(_)) if check(mode, &selection, target, scale, &shown) => 0,
                    (other, unsupported) => {
                        eprintln!("unexpected: {other:?} / scroll mode {unsupported:?}");
                        1
                    }
                };
                if let Some(mut child) = child {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                cx.update(|cx| cx.quit());
                eprintln!(
                    "{}",
                    if code == 0 {
                        "SELECTOR_OK"
                    } else {
                        "SELECTOR_FAIL"
                    }
                );
                std::process::exit(code);
            })
            .detach();
        });
}
