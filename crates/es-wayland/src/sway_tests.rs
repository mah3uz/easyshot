use crate::worker::Command;
use crate::{Connection, Error, Toplevel};
use es_geom::Point;
use es_model::platform::{
    CaptureOpts, ClipItem, CursorSource, Frame, OutputEvent, OutputId, OutputInfo, WindowEvent,
    WindowId,
};
use std::process::Command as Process;
use std::sync::Arc;
use std::time::{Duration, Instant};

const IGNORED: &str = "needs headless sway: tests/harness/sway.sh run -- cargo nextest run -p es-wayland -j 1 --run-ignored only -E 'test(/harness_sway_/)'";

pub(crate) fn harness_output() -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    // Refuse to touch a live session: only the harness compositor's runtime dir is accepted.
    assert!(
        std::env::var("EASYSHOT_TEST").as_deref() == Ok("1")
            && runtime.contains("easyshot-harness-"),
        "{IGNORED}"
    );
    std::env::var("EASYSHOT_TEST_OUTPUT").expect(IGNORED)
}

pub(crate) fn swaymsg(args: &[&str]) {
    let out = Process::new("swaymsg")
        .args(args)
        .output()
        .expect("swaymsg");
    assert!(out.status.success(), "swaymsg {args:?}: {out:?}");
}

fn next_event(conn: &Connection, mut want: impl FnMut(&OutputEvent) -> bool) -> OutputEvent {
    let rx = conn.output_events();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match rx.try_recv() {
            Ok(e) if want(&e) => return e,
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    panic!("no matching output event within 5 s");
}

fn output(conn: &Connection, name: &str) -> OutputInfo {
    conn.outputs()
        .into_iter()
        .find(|o| o.id.0 == name)
        .expect("harness output listed")
}

fn capture(conn: &Connection, name: &str) -> Frame {
    let t = Instant::now();
    let mut frames = conn
        .capture_outputs(&[OutputId(name.into())], CaptureOpts { cursor: false })
        .expect("capture");
    eprintln!("capture of {name} took {:?}", t.elapsed());
    assert_eq!(frames.len(), 1);
    frames.remove(0)
}

fn assert_frame_matches(frame: &Frame, info: &OutputInfo) {
    assert_eq!(frame.size_px, info.physical, "frame is in physical pixels");
    assert_eq!(frame.logical, info.logical);
    assert!((frame.scale - info.scale).abs() < 1e-9);
    assert_eq!(frame.output.as_ref(), Some(&info.id));
    let (w, h) = frame.size_px;
    assert_eq!(frame.bgra.len(), (w * h * 4) as usize);
    assert!(
        frame.bgra.as_chunks::<4>().0.iter().all(|p| p[3] == 255),
        "output frames are opaque"
    );
    assert!(
        frame
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[..3] != [0, 0, 0]),
        "frame has content"
    );
}

fn set_scale(conn: &Connection, name: &str, scale: &str, expect: f64) -> OutputInfo {
    let now = output(conn, name);
    if (now.scale - expect).abs() < 1e-9 {
        return now;
    }
    swaymsg(&["output", name, "scale", scale]);
    match next_event(
        conn,
        |e| matches!(e, OutputEvent::Changed(i) if i.id.0 == name && (i.scale - expect).abs() < 1e-9),
    ) {
        OutputEvent::Changed(i) => i,
        _ => unreachable!(),
    }
}

pub(crate) struct TestWindow {
    app_id: String,
    pub(crate) toplevel: Toplevel,
}

impl TestWindow {
    fn open(conn: &Connection, app_id: &str) -> Self {
        Self::running(conn, app_id, "")
    }

    pub(crate) fn running(conn: &Connection, app_id: &str, command: &str) -> Self {
        swaymsg(&["exec", &format!("alacritty --class {app_id} {command}")]);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let found = conn.toplevels().expect("toplevels");
            if let Some(toplevel) = found.into_iter().find(|t| t.app_id == app_id) {
                // Let the first frame land before anything captures it.
                std::thread::sleep(Duration::from_millis(300));
                return TestWindow {
                    app_id: app_id.to_owned(),
                    toplevel,
                };
            }
            assert!(Instant::now() < deadline, "test window did not appear");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = Process::new("swaymsg")
            .args([&format!("[app_id={}]", self.app_id), "kill"])
            .output();
    }
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_probe_and_capture_at_both_scales() {
    let name = harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let g = conn.globals();
    assert!(g.wlr_screencopy.is_some() && g.ext_output_capture() && g.xdg_output.is_some());
    assert!(g.layer_shell.is_some() && g.toplevel_capture());
    let _window = TestWindow::open(&conn, "easyshot-test-scales");

    let info = set_scale(&conn, &name, "1.5", 1.5);
    assert_eq!(info, output(&conn, &name));
    assert_eq!(info.physical, (1920, 1080));
    assert_eq!(info.logical.size(), (1280.0, 720.0).into());
    let frame = capture(&conn, &name);
    assert_frame_matches(&frame, &info);
    eprintln!(
        "scale 1.5 frame {:?} logical {:?}",
        frame.size_px, frame.logical
    );

    let info = set_scale(&conn, &name, "1", 1.0);
    assert_eq!(info.logical.size(), (1920.0, 1080.0).into());
    assert_frame_matches(&capture(&conn, &name), &info);

    set_scale(&conn, &name, "1.5", 1.5);
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_screencopy_fallback_matches_ext_capture() {
    let name = harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let _window = TestWindow::open(&conn, "easyshot-test-fallback");
    let info = output(&conn, &name);
    let ext = capture(&conn, &name);
    conn.send(Command::DisableExtCapture).expect("worker alive");
    let wlr = capture(&conn, &name);
    assert_frame_matches(&wlr, &info);
    assert_eq!(wlr.size_px, ext.size_px);
    assert_eq!(wlr.transform, ext.transform);
    assert!(wlr.bgra == ext.bgra, "both protocols give the same pixels");
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_hotplug_reports_added_and_removed() {
    harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let known: Vec<OutputId> = conn.outputs().into_iter().map(|o| o.id).collect();
    swaymsg(&["create_output"]);
    let added = match next_event(
        &conn,
        |e| matches!(e, OutputEvent::Added(i) if !known.contains(&i.id)),
    ) {
        OutputEvent::Added(i) => i,
        _ => unreachable!(),
    };
    assert!(added.physical.0 > 0 && added.scale > 0.0);
    assert!(conn.outputs().contains(&added));
    swaymsg(&["output", &added.id.0, "unplug"]);
    next_event(&conn, |e| *e == OutputEvent::Removed(added.id.clone()));
    assert!(conn.outputs().iter().all(|o| o.id != added.id));
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_window_capture_returns_the_window_pixels() {
    harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let window = TestWindow::open(&conn, "easyshot-test-window");
    let t = Instant::now();
    let frame = conn.capture_toplevel(&window.toplevel.identifier, CaptureOpts { cursor: false });
    eprintln!("window capture took {:?}", t.elapsed());
    let frame = frame.expect("window capture");
    let (w, h) = frame.size_px;
    assert!(w > 100 && h > 100, "{w}x{h}");
    assert_eq!(frame.output, None);
    let center = ((h / 2 * w + w / 2) * 4) as usize;
    assert_eq!(frame.bgra[center + 3], 255);
    assert!(matches!(
        conn.capture_toplevel("no-such-window", CaptureOpts { cursor: false }),
        Err(crate::Error::UnknownToplevel(_))
    ));
}

fn eventually<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = probe() {
            return v;
        }
        assert!(Instant::now() < deadline, "{what} within 5 s");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn next_window_event(
    rx: &async_channel::Receiver<WindowEvent>,
    mut want: impl FnMut(&WindowEvent) -> bool,
) -> WindowEvent {
    eventually("a matching window event", || {
        std::iter::from_fn(|| rx.try_recv().ok()).find(|e| want(e))
    })
}

fn paste(args: &[&str]) -> Vec<u8> {
    let out = Process::new("wl-paste")
        .arg("--no-newline")
        .args(args)
        .output()
        .expect("wl-paste");
    assert!(out.status.success(), "wl-paste {args:?}: {out:?}");
    out.stdout
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_window_events_follow_a_window_from_open_to_close() {
    harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let rx = conn.window_events();
    let window = TestWindow::open(&conn, "easyshot-test-events");
    let id = WindowId(window.toplevel.identifier.clone());
    match next_window_event(&rx, |e| matches!(e, WindowEvent::Opened(i) if i.id == id)) {
        WindowEvent::Opened(info) => {
            assert_eq!(info.app_id, "easyshot-test-events");
            assert_eq!(info.logical, None, "foreign-toplevel gives no geometry");
        }
        _ => unreachable!(),
    }
    next_window_event(&rx, |e| *e == WindowEvent::Focused(id.clone()));
    let listed = conn.windows().into_iter().find(|w| w.id == id);
    assert_eq!(listed.and_then(|w| w.focus_rank), Some(0));
    drop(window);
    next_window_event(&rx, |e| *e == WindowEvent::Closed(id.clone()));
    assert!(conn.windows().iter().all(|w| w.id != id));
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_clipboard_serves_every_type_over_both_protocols() {
    harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let g = conn.globals();
    assert!(g.ext_data_control.is_some() && g.wlr_data_control.is_some());
    let png: Arc<[u8]> = (0..1_000_000u32)
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>()
        .into();
    conn.set_clipboard(&[
        ClipItem::Png(png.clone()),
        ClipItem::Text("easyshot ✓".into()),
    ])
    .expect("set over ext-data-control");
    let types = String::from_utf8(paste(&["--list-types"])).expect("utf-8 types");
    for mime in ["image/png", "text/plain;charset=utf-8", "UTF8_STRING"] {
        assert!(types.lines().any(|l| l == mime), "{mime} in {types:?}");
    }
    assert!(*paste(&["--type", "image/png"]) == *png, "png bytes intact");
    assert_eq!(
        paste(&["--type", "text/plain;charset=utf-8"]),
        "easyshot ✓".as_bytes()
    );

    conn.send(Command::DisableExtDataControl)
        .expect("worker alive");
    conn.set_clipboard(&[ClipItem::UriList(vec!["/tmp/a b.png".into()])])
        .expect("set over wlr-data-control");
    assert_eq!(
        paste(&["--type", "text/uri-list"]),
        b"file:///tmp/a%20b.png\r\n"
    );
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_virtual_pointer_scrolls_a_window_and_the_cursor_session_follows() {
    let name = harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    let window = TestWindow::running(&conn, "easyshot-test-scroll", "--hold -e seq 1 3000");
    let shot = || {
        conn.capture_toplevel(&window.toplevel.identifier, CaptureOpts { cursor: false })
            .expect("window capture")
            .bgra
    };
    let screen = output(&conn, &name).logical;
    // Headless sway has no pointer until the first virtual one, and only then do clients bind theirs.
    conn.scroll(screen.center() + (50.0, 50.0), 0.0)
        .expect("create the pointer");
    std::thread::sleep(Duration::from_millis(300));
    let before = shot();
    assert!(shot() == before, "the window is still before scrolling");
    assert!(matches!(
        conn.scroll(Point::new(screen.x1 + 10.0, screen.y0), -150.0),
        Err(Error::OffScreen)
    ));
    conn.scroll(screen.center(), -150.0).expect("scroll");
    eventually("the terminal to scroll back", || {
        (shot() != before).then_some(())
    });

    let session = eventually("a cursor session", || conn.cursor_session().ok());
    let target = Point::new(screen.x0 + 100.0, screen.y0 + 200.0);
    conn.scroll(target, -15.0).expect("move and scroll");
    let (at, _) = eventually("the cursor sample to follow the pointer", || {
        session
            .sample()
            .filter(|(p, _)| (*p - target).hypot() < 1.0)
    });
    eprintln!("cursor sampled at {at:?}");
}
