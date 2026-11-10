use super::*;
use es_model::platform::WindowId;

fn on_screen() -> Start {
    let w = WindowInfo {
        id: WindowId("w1".into()),
        app_id: "app".into(),
        title: "title".into(),
        pid: None,
        logical: Some(Rect::new(0.0, 0.0, 100.0, 80.0)),
        output: Some(OutputId("A".into())),
        workspace: Some("1".into()),
        floating: false,
        focus_rank: None,
    };
    let mut start = display();
    start.request.target = RecordingTarget::Window(w);
    start
}

fn window_recorder(closes: bool) -> FakeRecorder {
    FakeRecorder {
        window: true,
        closes,
        ..FakeRecorder::default()
    }
}

fn recorder_in_capture_json(rig: &Rig) -> String {
    let folder = rig.folders().remove(0);
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("capture.json")).unwrap()).unwrap();
    capture["recorder"].as_str().unwrap().to_owned()
}

fn record(start: Start, rig: &Rig, cx: &mut TestAppContext) {
    cx.update(|cx| rig.recordings.start(start, cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();
}

#[gpui::test]
async fn a_window_recording_uses_the_window_recorder_when_there_is_one(cx: &mut TestAppContext) {
    let rig = rig_full(
        cx,
        FakeRecorder::default(),
        Some(window_recorder(false)),
        false,
    );
    record(on_screen(), &rig, cx);
    assert_eq!(recorder_in_capture_json(&rig), "window");
    assert_eq!(rig.starts.get(), 0, "the rectangle recorder is not asked");
}

#[gpui::test]
async fn a_window_recording_falls_back_to_the_rectangle_recorder(cx: &mut TestAppContext) {
    let rig = rig(cx);
    record(on_screen(), &rig, cx);
    assert_eq!(recorder_in_capture_json(&rig), "fake");
}

#[gpui::test]
async fn a_screen_recording_never_uses_the_window_recorder(cx: &mut TestAppContext) {
    let rig = rig_full(
        cx,
        FakeRecorder::default(),
        Some(window_recorder(false)),
        false,
    );
    record(display(), &rig, cx);
    assert_eq!(recorder_in_capture_json(&rig), "fake");
}

#[gpui::test]
async fn only_the_rectangle_recorder_dims_around_what_it_records(cx: &mut TestAppContext) {
    let with_window = rig_full(
        cx,
        FakeRecorder::default(),
        Some(window_recorder(false)),
        false,
    );
    record(on_screen(), &with_window, cx);
    assert_eq!(with_window.log.borrow().dimmed, [false]);

    let rectangle = rig(cx);
    record(on_screen(), &rectangle, cx);
    assert_eq!(rectangle.log.borrow().dimmed, [true]);
}

/// Records until the window recorder reports the window gone; the toasts shown, as title and message.
fn record_until_the_window_ends(
    recorder: FakeRecorder,
    cx: &mut TestAppContext,
) -> Vec<(String, Option<String>)> {
    let rig = rig_full(cx, FakeRecorder::default(), Some(recorder), false);
    let sink = es_app::toast::MemorySink::new();
    cx.update(|cx| {
        es_app::toast::install_toasts(
            sink.clone(),
            Arc::new(es_desktop::RecordingNotifier::new(false)),
            cx,
        )
    });
    cx.update(|cx| rig.recordings.start(on_screen(), cx))
        .unwrap();
    cx.run_until_parked();

    assert!(!rig.recordings.is_recording());
    let cards = cards(&rig, cx);
    assert_eq!(cards.len(), 1, "the recording is kept");
    assert!(cards[0].video);
    sink.shown()
        .into_iter()
        .map(|t| (t.title, t.message))
        .collect()
}

#[gpui::test]
async fn a_closed_window_ends_the_recording_keeps_it_and_says_so(cx: &mut TestAppContext) {
    let shown = record_until_the_window_ends(window_recorder(true), cx);
    assert!(shown.iter().any(|(title, message)| {
        title == "The window closed"
            && message.as_deref() == Some("The recording stopped. It’s saved.")
    }));
}

#[gpui::test]
async fn a_window_that_can_no_longer_be_captured_ends_the_recording_keeps_it_and_says_so(
    cx: &mut TestAppContext,
) {
    let recorder = FakeRecorder {
        lost: true,
        ..window_recorder(false)
    };
    let shown = record_until_the_window_ends(recorder, cx);
    assert!(shown.iter().any(|(title, message)| {
        title == "The window can’t be recorded any more"
            && message.as_deref() == Some("The recording stopped. It’s saved.")
    }));
}
