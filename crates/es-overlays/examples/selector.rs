use es_app::platform_ui::{LayerShellSurfaces, SelectMode, Selection, SelectionFlow};
use es_backend::{Backend, Probe};
use es_geom::{Point, Rect};
use es_model::platform::WindowInfo;
use es_overlays::capture::LayerShellSelectionFlow;
use es_overlays::capture::color::hex;
use es_overlays::capture::scroll_hud::{HUD, hud_origin};
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
const SCROLL_AREA: (i32, i32, i32, i32) = (8, 40, 640, 520);
const SCROLL_LINES: u32 = 150;
const STOP_PILL: (f64, f64) = (364.0, 18.0);

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Region,
    Window,
    Color,
    Scroll,
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
fn terminal(backend: &Backend, mode: Mode) -> (Child, WindowInfo) {
    let pager = format!("seq -f 'line %04g' 1 {SCROLL_LINES} | less");
    let run: &[&str] = if mode == Mode::Scroll {
        &["sh", "-c", &pager]
    } else {
        sway(&["for_window [app_id=\"Alacritty\"] floating enable"]);
        &["sleep", "600"]
    };
    let mut child = Command::new("alacritty")
        .args([
            "-o",
            &format!("colors.primary.background=\"{TERMINAL_BG}\""),
            "-e",
        ])
        .args(run)
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
fn drive(backend: &Backend, output: (&str, Rect), shots: &Path, mode: Mode, target: Option<Rect>) {
    let (output, visible) = output;
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
        (Mode::Scroll, _) => {
            let (x0, y0, x1, y1) = SCROLL_AREA;
            to(x0, y0);
            cursor("press button1");
            for step in 1..=4 {
                to(x0 + (x1 - x0) * step / 4, y0 + (y1 - y0) * step / 4);
            }
            cursor("release button1");
            pause(300);
            to((x0 + x1) / 2, (y0 + y1) / 2);
            for _ in 0..2 {
                click();
            }
            pause(4000);
            let centre = Point::new(f64::from(x0 + x1) / 2., f64::from(y0 + y1) / 2.);
            for step in 0..45 {
                if let Some(input) = backend.virtual_input.as_ref()
                    && let Err(e) = input.scroll(centre, 15.0)
                {
                    eprintln!("virtual wheel: {e}");
                }
                pause(350);
                if step == 20 {
                    grim(output, &shots.join("harness-scroll-hud.png"));
                }
            }
            pause(800);
            let area = Rect::new(f64::from(x0), f64::from(y0), f64::from(x1), f64::from(y1));
            let hud = hud_origin(area, visible);
            debug_assert!(STOP_PILL.0 < HUD.0);
            to((hud.x + STOP_PILL.0) as i32, (hud.y + STOP_PILL.1) as i32);
            pause(200);
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

fn stitched(shots: &Path, logical: Rect, frame: &es_model::platform::Frame, scale: f64) -> bool {
    let (w, h) = frame.size_px;
    let rgba: Vec<u8> = frame
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[b, g, r, a]| [r, g, b, a])
        .collect();
    let path = shots.join("harness-scroll-stitched.png");
    if let Some(image) = image::RgbaImage::from_raw(w, h, rgba)
        && let Err(e) = image.save(&path)
    {
        eprintln!("write {}: {e}", path.display());
    }
    let (x0, y0, x1, y1) = SCROLL_AREA;
    let area_px = (
        (f64::from(x1 - x0) * scale).round() as u32,
        (f64::from(y1 - y0) * scale).round() as u32,
    );
    eprintln!(
        "STITCHED {} px={:?} logical={logical:?} area_px={area_px:?}",
        path.display(),
        frame.size_px
    );
    w.abs_diff(area_px.0) <= 1 && h > area_px.1 * 2
}

fn check(
    mode: Mode,
    selection: &Selection,
    target: Option<Rect>,
    scale: f64,
    shown: &str,
    shots: &Path,
) -> bool {
    match (mode, selection) {
        (Mode::Scroll, Selection::Region { logical, frame, .. }) => {
            stitched(shots, *logical, frame, scale)
        }
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
        .unwrap_or_else(|| fail("usage: selector SCREENSHOT_DIR [region|window|color|scroll]"));
    let mode = match std::env::args().nth(2).as_deref() {
        None | Some("region") => Mode::Region,
        Some("window") => Mode::Window,
        Some("color") => Mode::Color,
        Some("scroll") => Mode::Scroll,
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
        let (c, w) = terminal(&backend, mode);
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
        let (backend, output, shots) = (backend.clone(), test_output.0.clone(), shots.clone());
        let visible = info.logical;
        move || drive(&backend, (&output, visible), &shots, mode, target)
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
                    Mode::Scroll => SelectMode::Scroll,
                },
                cx,
            );
            cx.spawn(async move |cx| {
                let code = match task.await {
                    Ok(selection) if check(mode, &selection, target, scale, &shown, &shots) => 0,
                    other => {
                        eprintln!("unexpected: {other:?}");
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
