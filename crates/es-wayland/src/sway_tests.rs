use crate::worker::Command;
use crate::{Connection, Toplevel};
use es_model::platform::{CaptureOpts, Frame, OutputEvent, OutputId, OutputInfo};
use std::process::Command as Process;
use std::time::{Duration, Instant};

const IGNORED: &str = "needs headless sway: tests/harness/sway.sh run -- cargo nextest run -p es-wayland -j 1 --run-ignored only -E 'test(/harness_sway_/)'";

fn harness_output() -> String {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    // Refuse to touch a live session: only the harness compositor's runtime dir is accepted.
    assert!(
        std::env::var("EASYSHOT_TEST").as_deref() == Ok("1")
            && runtime.contains("easyshot-harness-"),
        "{IGNORED}"
    );
    std::env::var("EASYSHOT_TEST_OUTPUT").expect(IGNORED)
}

fn swaymsg(args: &[&str]) {
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

struct TestWindow {
    app_id: String,
    toplevel: Toplevel,
}

impl TestWindow {
    fn open(conn: &Connection, app_id: &str) -> Self {
        swaymsg(&["exec", &format!("alacritty --class {app_id}")]);
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
