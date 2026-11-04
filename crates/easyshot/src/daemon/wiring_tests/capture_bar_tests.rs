use super::*;

#[gpui::test]
fn the_capture_bar_lists_devices_and_windows_and_refresh_asks_again(cx: &mut TestAppContext) {
    use std::cell::Cell;
    let rig = rig(cx);
    let (windows, devices) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (w, d) = (windows.clone(), devices.clone());
    rig.wiring.set_bar_sources(BarSources {
        windows: Rc::new(move || {
            w.set(w.get() + 1);
            Vec::new()
        }),
        devices: Rc::new(move || {
            d.set(d.get() + 1);
            Vec::new()
        }),
        recorder_missing: false,
    });
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    assert_eq!((windows.get(), devices.get()), (1, 1), "listed at open");
    bar.update(cx, |_, cx| cx.emit(BarEvent::RefreshWindows));
    assert_eq!(windows.get(), 2, "Refresh Windows lists the windows again");
    assert_eq!(devices.get(), 1);
}

#[gpui::test]
fn the_capture_bar_is_told_when_no_recorder_is_installed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sources = BarSources {
        recorder_missing: true,
        ..BarSources::default()
    };
    rig.wiring.set_bar_sources(sources);
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    let hint = cx.update(|cx| bar.read(cx).recording_hint());
    assert!(hint.contains("No screen recorder is installed"), "{hint}");
}

#[gpui::test]
fn the_capture_bar_panel_starts_from_the_recording_settings(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings.update(cx, |s, cx| {
        s.update(
            |c| {
                c.recording.mic = "default".into();
                c.recording.system_audio = true;
                c.recording.start_delay = 3;
            },
            cx,
        )
    });
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    let request = cx.update(|cx| {
        let options = bar.read(cx).options().clone();
        options
            .read(cx)
            .request(es_overlays::record::RecordingTarget::Area)
    });
    assert_eq!(request.mic.as_deref(), Some("default"));
    assert!(request.system_audio);
    assert_eq!(request.delay_secs, 3);
}

struct Pactl(&'static str);

impl es_record::audio::Runner for Pactl {
    fn run(&self, _: &str, _: &[&str]) -> std::io::Result<String> {
        Ok(self.0.to_owned())
    }
}

#[test]
fn audio_devices_come_through_the_runner_and_fail_to_none() {
    let listed = devices_from(Rc::new(Pactl("[]")))();
    assert!(
        listed
            .iter()
            .any(|d| d.kind == es_record::AudioKind::Microphone),
        "{listed:?}"
    );
    assert!(devices_from(Rc::new(NoAudio))().is_empty());
}
