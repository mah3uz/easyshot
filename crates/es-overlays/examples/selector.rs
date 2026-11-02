use es_app::platform_ui::{LayerShellSurfaces, SelectMode, Selection, SelectionFlow};
use es_backend::{Backend, Probe};
use es_geom::Point;
use es_overlays::capture::LayerShellSelectionFlow;
use gpui::{App, QuitMode};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Odd logical edges at scale 1.5 land on half pixels, where rounding origin and size separately goes wrong.
const FROM: (i32, i32) = (101, 51);
const TO: (i32, i32) = (304, 152);

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

// `swaymsg seat cursor set` sends no motion while a button is held, so motion comes from the virtual pointer.
fn drive(backend: &Backend, output: &str, shots: &Path) {
    let pause = |ms| thread::sleep(Duration::from_millis(ms));
    let to = |x: i32, y: i32| {
        if let Some(input) = backend.virtual_input.as_ref()
            && let Err(e) = input.scroll(Point::new(f64::from(x), f64::from(y)), 0.0)
        {
            eprintln!("virtual pointer: {e}");
        }
        pause(40);
    };
    pause(5000);
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
        cursor("press button1");
        cursor("release button1");
    }
}

fn expected() -> (u32, u32) {
    let edge = |v: i32| (f64::from(v) * 1.5).round() as u32;
    (edge(TO.0) - edge(FROM.0), edge(TO.1) - edge(FROM.1))
}

fn main() {
    let shots = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("usage: selector SCREENSHOT_DIR"));
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
    thread::spawn({
        let (backend, output) = (backend.clone(), test_output.0.clone());
        move || drive(&backend, &output, &shots)
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
            let task = flow.run(SelectMode::Region, cx);
            let unsupported = flow.run(SelectMode::Color, cx);
            cx.spawn(async move |cx| {
                let code = match (task.await, unsupported.await) {
                    (
                        Ok(Selection::Region {
                            output,
                            logical,
                            frame,
                        }),
                        Err(_),
                    ) => {
                        eprintln!(
                            "SELECTED {} logical={logical:?} px={:?} expected={:?}",
                            output.0,
                            frame.size_px,
                            expected()
                        );
                        if frame.size_px == expected() { 0 } else { 1 }
                    }
                    (other, unsupported) => {
                        eprintln!("unexpected: {other:?} / colour mode {unsupported:?}");
                        1
                    }
                };
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
