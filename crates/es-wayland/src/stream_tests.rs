use crate::sway_tests::{TestWindow, harness_output, swaymsg};
use crate::{Connection, Error, StreamEvent, StreamOptions, ToplevelStream};
use es_model::VideoFrame;
use std::process::Command;
use std::time::{Duration, Instant};

// Prints a counter about 200 times a second, so every frame the terminal draws differs.
const CHANGING: &str = "-e sh -c 'i=0; while :; do i=$((i+1)); echo $i; sleep 0.005; done'";
const UNTHROTTLED: StreamOptions = StreamOptions {
    fps: 0,
    paint_cursor: false,
};

fn next_frame(stream: &ToplevelStream, within: Duration) -> VideoFrame {
    match stream.recv_timeout(within) {
        Some(StreamEvent::Frame(f)) => f,
        other => panic!("expected a frame, got {other:?}"),
    }
}

fn assert_well_formed(f: &VideoFrame) {
    assert_eq!(f.stride, f.width * 4, "rows are packed");
    assert_eq!(f.data.len(), (f.stride * f.height) as usize);
}

#[derive(Debug)]
struct Counted {
    frames: u32,
    changed: u32,
}

/// Counts the frames that arrive in `span` and how many differ from the frame before.
fn count(stream: &ToplevelStream, span: Duration, last: &mut Option<VideoFrame>) -> Counted {
    let mut c = Counted {
        frames: 0,
        changed: 0,
    };
    let end = Instant::now() + span;
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match stream.recv_timeout(left) {
            Some(StreamEvent::Frame(f)) => {
                c.frames += 1;
                if last.as_ref().is_none_or(|l| l.data[..] != f.data[..]) {
                    c.changed += 1;
                }
                *last = Some(f);
            }
            Some(other) => panic!("stream ended: {other:?}"),
            None => {}
        }
    }
    c
}

/// The first frame whose size passes `want`, after checking every frame on the way is well formed.
fn wait_for_size(stream: &ToplevelStream, want: impl Fn((u32, u32)) -> bool) -> VideoFrame {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut seen = Vec::new();
    loop {
        let f = next_frame(stream, Duration::from_secs(2));
        assert_well_formed(&f);
        if want((f.width, f.height)) {
            return f;
        }
        if seen.last() != Some(&(f.width, f.height)) {
            seen.push((f.width, f.height));
        }
        assert!(Instant::now() < deadline, "no wanted size, saw {seen:?}");
    }
}

fn open_window(app_id: &str) -> (Connection, TestWindow) {
    harness_output();
    let conn = Connection::connect().expect("connect to harness sway");
    swaymsg(&["workspace", "1"]);
    let window = TestWindow::running(&conn, app_id, CHANGING);
    (conn, window)
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_stream_frames_change_and_keep_coming_from_a_hidden_workspace() {
    let (_conn, window) = open_window("es-stream-hidden");
    let stream = ToplevelStream::open(&window.toplevel.identifier, UNTHROTTLED).expect("stream");
    let mut last = None;
    let visible = count(&stream, Duration::from_secs(1), &mut last);
    assert!(visible.frames > 20 && visible.changed > 20, "{visible:?}");
    assert_well_formed(last.as_ref().expect("a frame"));

    swaymsg(&["[app_id=es-stream-hidden]", "move", "to", "workspace", "9"]);
    count(&stream, Duration::from_millis(300), &mut last);
    let hidden = count(&stream, Duration::from_secs(1), &mut last);
    eprintln!(
        "visible {visible:?}, hidden {hidden:?}, {:?}",
        stream.stats()
    );
    assert!(hidden.frames > 20 && hidden.changed > 20, "{hidden:?}");
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_stream_follows_a_resize_with_frames_of_the_new_size() {
    let (_conn, window) = open_window("es-stream-resize");
    let target = "[app_id=es-stream-resize]";
    swaymsg(&[target, "floating", "enable"]);
    // A resize sent while the window is still turning floating is lost to its floating default.
    std::thread::sleep(Duration::from_millis(300));
    swaymsg(&[target, "resize", "set", "400", "300"]);
    let stream = ToplevelStream::open(&window.toplevel.identifier, UNTHROTTLED).expect("stream");
    wait_for_size(&stream, |s| s == (400, 300));
    let before = stream.stats().shm_buffers;

    swaymsg(&[target, "resize", "set", "640", "480"]);
    wait_for_size(&stream, |s| s == (640, 480));
    let stats = stream.stats();
    eprintln!("{stats:?}");
    assert!(stats.shm_buffers > before, "a new buffer for the new size");
    let f = next_frame(&stream, Duration::from_secs(1));
    assert_eq!((f.width, f.height), (640, 480));
    assert_well_formed(&f);
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_stream_reports_stopped_within_a_second_of_the_window_closing() {
    let (_conn, window) = open_window("es-stream-close");
    let options = StreamOptions {
        fps: 30,
        paint_cursor: true,
    };
    let stream = ToplevelStream::open(&window.toplevel.identifier, options).expect("stream");
    next_frame(&stream, Duration::from_secs(2));
    let closed_at = Instant::now();
    drop(window);
    loop {
        match stream.recv_timeout(Duration::from_secs(2)) {
            Some(StreamEvent::Frame(_)) => {}
            Some(StreamEvent::Stopped) => break,
            other => panic!("expected stopped, got {other:?}"),
        }
    }
    let after = closed_at.elapsed();
    eprintln!("stopped {after:?} after close; {:?}", stream.stats());
    assert!(after < Duration::from_secs(1), "stopped after {after:?}");
    let t = Instant::now();
    assert!(stream.recv_timeout(Duration::from_secs(2)).is_none());
    assert!(
        t.elapsed() < Duration::from_millis(100),
        "an ended stream does not wait"
    );
    assert!(matches!(
        ToplevelStream::open("no-such-window", UNTHROTTLED),
        Err(Error::UnknownToplevel(_))
    ));
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_stream_requests_frames_no_faster_than_the_target_fps() {
    let (_conn, window) = open_window("es-stream-throttle");
    let rate = |fps| {
        let options = StreamOptions {
            fps,
            paint_cursor: false,
        };
        let stream = ToplevelStream::open(&window.toplevel.identifier, options).expect("stream");
        next_frame(&stream, Duration::from_secs(2));
        let (start, t) = (stream.stats(), Instant::now());
        std::thread::sleep(Duration::from_secs(3));
        let (end, secs) = (stream.stats(), t.elapsed().as_secs_f64());
        (
            (end.requests - start.requests) as f64 / secs,
            (end.frames - start.frames) as f64 / secs,
        )
    };
    let (requests, frames) = rate(30);
    let (free, _) = rate(0);
    eprintln!("30 fps: {requests:.1} requests/s, {frames:.1} frames/s; unthrottled {free:.1}/s");
    assert!(requests <= 33.0, "{requests:.1} requests/s");
    assert!(frames >= 20.0, "{frames:.1} frames/s");
    // Without this the window could simply be too slow to show the throttle.
    assert!(free > 45.0, "unthrottled {free:.1}/s");
}

#[test]
#[ignore = "headless sway harness"]
fn harness_sway_stream_reuses_its_pixel_buffers() {
    let (_conn, window) = open_window("es-stream-pool");
    let stream = ToplevelStream::open(&window.toplevel.identifier, UNTHROTTLED).expect("stream");
    // Held like a pacer holds the last frame to repeat it.
    let mut held = None;
    for _ in 0..300 {
        held = Some(next_frame(&stream, Duration::from_secs(2)));
    }
    let stats = stream.stats();
    eprintln!("{stats:?}");
    assert!(held.is_some());
    // The frame being filled, the mailbox, the held frame and one being swapped for it.
    assert!(stats.allocations <= 4, "{stats:?}");
    let t = Instant::now();
    stream.stop();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "stop took {:?}",
        t.elapsed()
    );
}

fn hypr_step(phase: &str) {
    let step = std::env::var("EASYSHOT_PROBE_STEP").expect("EASYSHOT_PROBE_STEP");
    let out = Command::new(&step).arg(phase).output().expect("step runs");
    eprint!("{}", String::from_utf8_lossy(&out.stdout));
    assert!(
        out.status.success(),
        "step {phase}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
#[ignore = "Hyprland test output: tests/harness/hypr-stream-probe.sh"]
fn hypr_stream_probe() {
    assert!(
        std::env::var("EASYSHOT_TEST").as_deref() == Ok("1")
            && std::env::var("EASYSHOT_TEST_OUTPUT").as_deref() == Ok("easyshot-test"),
        "run through tests/harness/hypr-stream-probe.sh"
    );
    let app_id = std::env::var("EASYSHOT_PROBE_APP_ID").expect("EASYSHOT_PROBE_APP_ID");
    let conn = Connection::connect().expect("connect to Hyprland");
    let deadline = Instant::now() + Duration::from_secs(10);
    let identifier = loop {
        let found = conn.toplevels().expect("toplevels");
        if let Some(t) = found.into_iter().find(|t| t.app_id == app_id) {
            break t.identifier;
        }
        assert!(Instant::now() < deadline, "no window with app id {app_id}");
        std::thread::sleep(Duration::from_millis(100));
    };
    let stream = ToplevelStream::open(&identifier, UNTHROTTLED).expect("stream");
    let span = Duration::from_secs(3);
    let mut last = None;
    for phase in [
        "visible",
        "hidden-workspace",
        "covered-by-fullscreen",
        "visible-again",
    ] {
        hypr_step(phase);
        count(&stream, Duration::from_millis(400), &mut last);
        let c = count(&stream, span, &mut last);
        let fps = f64::from(c.frames) / span.as_secs_f64();
        eprintln!("RESULT {phase} fps {fps:.1} changed {}", c.changed);
        assert!(c.frames > 20 && c.changed > 20, "{phase}: {c:?}");
    }
    eprintln!("{:?}", stream.stats());
}
