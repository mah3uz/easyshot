use es_app::deck::{Deck, DeckItem, DeckPolicy, ItemStatus};
use es_app::platform_ui::LayerShellSurfaces;
use es_backend::{Backend, Probe};
use es_geom::{Point, Rect, Size};
use es_model::CaptureKind;
use es_overlays::deck::view::{DeckSettings, DeckSurface, load_thumbnail};
use es_overlays::pin::{PinEvent, Pins, initial_origin, initial_size};
use gpui::{App, AppContext as _, QuitMode};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const PIN_PX: (u32, u32) = (600, 400);
const DRAG: (f64, f64) = (150., 100.);

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("deck: {message}");
    std::process::exit(1);
}

fn cursor(action: &str) {
    let ok = Command::new("swaymsg")
        .args(["seat", "seat0", "cursor", action, "button1"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("swaymsg cursor {action} failed");
    }
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

fn picture(path: &Path, (w, h): (u32, u32), n: u32) {
    let image = image::RgbaImage::from_fn(w, h, |x, y| match (x < w / 6 * (n + 1), (y / 20) % 2) {
        (true, _) => image::Rgba([40, 110, 230, 255]),
        (false, 0) => image::Rgba([250, 250, 252, 255]),
        _ => image::Rgba([225, 229, 236, 255]),
    });
    if let Err(e) = image.save(path) {
        fail(format!("{}: {e}", path.display()));
    }
}

fn item(dir: &Path, n: u32) -> DeckItem {
    let thumb = dir.join(format!("thumb{n}.png"));
    picture(&thumb, (360, 220), n);
    DeckItem {
        id: Uuid::from_u128(u128::from(n)),
        name: format!("shot{n}"),
        file: thumb.clone(),
        thumb,
        kind: CaptureKind::Region,
        video: false,
        staged: true,
        status: ItemStatus::Idle,
        auto_dismiss: true,
    }
}

// `swaymsg seat cursor set` sends no motion while a button is held, so motion comes from the virtual pointer.
fn drive(backend: &Backend, output: &str, shots: &Path, logical: Rect, scale: f64) {
    let pause = |ms| thread::sleep(Duration::from_millis(ms));
    let to = |p: Point, dy: f64| {
        if let Some(input) = backend.virtual_input.as_ref()
            && let Err(e) = input.scroll(p, dy)
        {
            eprintln!("virtual pointer: {e}");
        }
        pause(40);
    };
    pause(5000);
    grim(output, &shots.join("deck-rest.png"));

    let geometry = DeckSettings::default().geometry(3);
    let panel = geometry.origin(logical);
    let card = geometry.card(2);
    to(
        Point::new(
            panel.x + card.center().x + 30.,
            panel.y + card.center().y + 30.,
        ),
        0.,
    );
    pause(600);
    eprintln!("STEP hover");
    grim(output, &shots.join("deck-hover.png"));

    let natural = Size::new(f64::from(PIN_PX.0) / scale, f64::from(PIN_PX.1) / scale);
    let size = initial_size(natural);
    let origin = initial_origin(logical, size, 0);
    let grab = Point::new(origin.x + size.width / 2., origin.y + size.height / 2.);
    let over = origin + (20., 20.);
    for _ in 0..4 {
        to(over, -10.);
        pause(80);
    }
    pause(600);
    eprintln!("STEP resized");
    grim(output, &shots.join("pin-resized.png"));
    to(grab, 0.);
    pause(300);
    cursor("press");
    // Faster than a frame per step, so motion arrives while a move is still waiting to be drawn.
    for step in 1..=30 {
        let f = f64::from(step) / 30.;
        let at = grab + (DRAG.0 * f, DRAG.1 * f);
        if let Some(input) = backend.virtual_input.as_ref()
            && let Err(e) = input.scroll(at, 0.)
        {
            eprintln!("virtual pointer: {e}");
        }
        pause(4);
    }
    pause(100);
    cursor("release");
    pause(600);
    eprintln!("STEP moved expect={:?}", origin + DRAG);
    grim(output, &shots.join("pin-moved.png"));
}

fn main() {
    let shots = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| fail("usage: deck SCREENSHOT_DIR"));
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
    if let Err(e) = input.scroll(Point::new(info.logical.x0 + 10., info.logical.y0 + 10.), 0.) {
        fail(format!("virtual pointer: {e}"));
    }
    let done = Arc::new(AtomicBool::new(false));
    thread::spawn({
        let (backend, output, shots, done) = (
            backend.clone(),
            test_output.0.clone(),
            shots.clone(),
            done.clone(),
        );
        let (logical, scale) = (info.logical, info.scale);
        move || {
            drive(&backend, &output, &shots, logical, scale);
            done.store(true, Ordering::SeqCst);
        }
    });
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            es_ui::hello::init(cx);
            let surfaces = Rc::new(LayerShellSurfaces::new(backend, Some(test_output)));
            let deck = cx.new(|_| {
                Deck::new(DeckPolicy {
                    delay: None,
                    keep_until_saved: false,
                })
            });
            let shown = DeckSurface::attach(surfaces.clone(), &deck, DeckSettings::default(), cx);
            deck.update(cx, |d, cx| {
                (1..=3).for_each(|n| d.show(item(&shots, n), cx))
            });

            let file = shots.join("pinned.png");
            picture(&file, PIN_PX, 2);
            let pins = Pins::new(
                surfaces,
                Rc::new(|path, _| eprintln!("COPY {}", path.display())),
            );
            let pin = load_thumbnail(&file)
                .ok_or_else(|| "pinned picture".to_string())
                .and_then(|image| pins.pin(file, image, cx).map_err(|e| e.to_string()))
                .unwrap_or_else(|e| fail(format!("pin: {e}")));
            let watched = pin.clone();
            let events = cx.subscribe(&pin, move |_, e: &PinEvent, cx| {
                let v = watched.read(cx);
                eprintln!("PIN {e:?} origin={:?} size={:?}", v.origin(), v.size());
            });
            cx.spawn(async move |cx| {
                let mut hovered = false;
                while !done.load(Ordering::SeqCst) {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let now = cx.update(|cx| deck.read(cx).hovered());
                    if now != hovered {
                        hovered = now;
                        eprintln!("DECK hovered={now}");
                    }
                }
                cx.update(|cx| {
                    eprintln!(
                        "DONE deck={} shown={} pins={}",
                        deck.read(cx).len(),
                        shown.is_shown(),
                        pins.count()
                    );
                    drop(events);
                    pins.unpin_all(cx);
                    cx.quit();
                });
            })
            .detach();
        });
}
