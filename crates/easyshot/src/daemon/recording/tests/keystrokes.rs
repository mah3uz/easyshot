#![allow(clippy::unwrap_used)]
use super::*;

const CTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_C: u16 = 46;
const ENTER: u16 = 28;
const ESC: u16 = 1;

impl Rig {
    fn keystrokes(&self, on: bool, cx: &mut TestAppContext) {
        let settings = cx.update(|_| self.recordings.0.deps.settings.clone());
        cx.update(|cx| settings.update(cx, |s, cx| s.update(|c| c.recording.keystrokes = on, cx)));
    }

    fn now_us(&self) -> u64 {
        self.clock.monotonic_us()
    }

    /// Hands events to the collector's thread and waits until it has taken them.
    fn feed(&self, events: &[(u16, bool)]) {
        let t_us = self.now_us();
        self.input
            .queue
            .lock()
            .unwrap()
            .extend(events.iter().map(|&(code, down)| {
                if code >= 0x110 {
                    es_record::RawEvent::Button { code, down, t_us }
                } else {
                    es_record::RawEvent::Key {
                        device: 0,
                        code,
                        down,
                        t_us,
                    }
                }
            }));
        while !self.input.queue.lock().unwrap().is_empty() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn input_json(&self) -> serde_json::Value {
        let folder = self.folders().remove(0);
        serde_json::from_slice(&std::fs::read(folder.join(es_record::project::INPUT)).unwrap())
            .unwrap()
    }
}

fn keys(v: &serde_json::Value) -> Vec<(f64, String)> {
    v["keystrokes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            let mods: Vec<&str> = k["modifiers"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|m| m.as_str())
                .collect();
            (
                k["time"].as_f64().unwrap(),
                format!("{}{}", mods.concat(), k["key"].as_str().unwrap()),
            )
        })
        .collect()
}

#[gpui::test]
async fn with_keystrokes_on_clicks_and_shortcuts_land_in_input_json_on_the_movie_clock(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.keystrokes(true, cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    rig.advance(1);
    rig.feed(&[(0x110, true), (0x110, false)]);
    rig.feed(&[(KEY_A, true), (KEY_A, false)]);
    rig.feed(&[(CTRL, true), (KEY_C, true), (KEY_C, false), (CTRL, false)]);
    rig.advance(1);
    cx.update(|cx| rig.recordings.toggle_pause(cx)).unwrap();
    cx.run_until_parked();
    rig.advance(3);
    rig.feed(&[(ENTER, true), (ENTER, false)]);
    rig.advance(7);
    cx.update(|cx| rig.recordings.toggle_pause(cx)).unwrap();
    cx.run_until_parked();
    rig.advance(1);
    rig.feed(&[(ESC, true), (ESC, false)]);
    rig.advance(1);
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();

    assert_eq!(rig.input.opened.get(), 1);
    let v = rig.input_json();
    assert_eq!(
        keys(&v),
        [(1.0, "⌃C".to_owned()), (3.0, "Esc".to_owned())],
        "plain typing and the Enter pressed while paused are left out; 10 s of pause removed"
    );
    let presses = v["presses"].as_array().unwrap();
    assert_eq!(presses.len(), 2);
    assert_eq!(
        (presses[0]["time"].as_f64(), &presses[0]["phase"]),
        (Some(1.0), &serde_json::json!("down"))
    );
    assert_eq!(v["travel"], serde_json::json!([]));
}

#[gpui::test]
async fn with_keystrokes_off_no_device_is_opened_and_input_json_is_empty(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.keystrokes(false, cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    rig.advance(1);
    rig.input
        .queue
        .lock()
        .unwrap()
        .push(es_record::RawEvent::Button {
            code: 0x110,
            down: true,
            t_us: rig.now_us(),
        });
    rig.advance(1);
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();

    assert_eq!(rig.input.opened.get(), 0, "the collector never starts");
    let v = rig.input_json();
    assert_eq!(v["presses"], serde_json::json!([]));
    assert_eq!(v["keystrokes"], serde_json::json!([]));
}

#[gpui::test]
async fn without_device_access_the_recording_runs_with_a_hint_and_an_empty_log(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let sink = es_app::toast::MemorySink::new();
    cx.update(|cx| {
        es_app::toast::install_toasts(
            sink.clone(),
            Arc::new(es_desktop::RecordingNotifier::new(false)),
            cx,
        )
    });
    rig.keystrokes(true, cx);
    rig.input.denied.set(true);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    assert!(rig.recordings.is_recording());
    rig.advance(2);
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();

    let shown = sink.shown();
    assert_eq!(shown.len(), 1);
    assert!(
        shown[0]
            .message
            .as_deref()
            .is_some_and(|m| m.contains("sudo usermod -aG input $USER"))
    );
    assert_eq!(cards(&rig, cx).len(), 1, "the recording is kept");
    assert_eq!(rig.input_json()["keystrokes"], serde_json::json!([]));
}

#[gpui::test]
async fn discard_and_switching_keystrokes_off_stop_reading_at_once(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let unread = |rig: &Rig| {
        let t_us = rig.now_us();
        let key = |down| es_record::RawEvent::Button {
            code: 0x110,
            down,
            t_us,
        };
        for _ in 0..2 {
            rig.input
                .queue
                .lock()
                .unwrap()
                .extend([key(true), key(false)]);
            std::thread::sleep(Duration::from_millis(100));
        }
        rig.input.queue.lock().unwrap().len()
    };
    rig.keystrokes(true, cx);
    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| rig.recordings.discard(cx)).unwrap();
    assert!(unread(&rig) >= 2, "read after Discard");
    cx.run_until_parked();
    rig.input.queue.lock().unwrap().clear();

    cx.update(|cx| rig.recordings.start(display(), cx)).unwrap();
    cx.run_until_parked();
    rig.keystrokes(false, cx);
    cx.run_until_parked();
    assert!(unread(&rig) >= 2, "read after Keystrokes was switched off");
    cx.update(|cx| rig.recordings.stop(cx)).unwrap();
    cx.run_until_parked();
    assert_eq!(rig.input_json()["presses"], serde_json::json!([]));
}
