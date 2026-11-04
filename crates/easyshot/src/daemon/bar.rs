use super::recording::Recordings;
use super::windows::Wiring;
use es_app::capture_coordinator::Hook;
use es_app::errors::Error;
use es_app::toast::{Toast, show_toast};
use es_app::windows::{WindowKey, WindowKind, Windows};
use es_model::platform::WindowInfo;
use es_overlays::record::bar::{BarEvent, CaptureBar};
use es_overlays::record::options::{RecordingDefaults, RecordingRequest};
use es_record::AudioDevice;
use es_record::audio::{Runner, list_audio_devices};
use gpui::{App, AppContext as _, Entity};
use std::rc::Rc;

/// A capture started while the bar is open would otherwise show the bar in the frame.
pub fn close_before_capture(windows: &Windows) -> Hook {
    let windows = windows.clone();
    Rc::new(move |cx| windows.close(WindowKind::CaptureBar, &WindowKey::Single, cx))
}

/// What the capture bar lists: windows to record and microphones to record with.
#[derive(Clone)]
pub struct BarSources {
    pub windows: Rc<dyn Fn() -> Vec<WindowInfo>>,
    pub devices: Rc<dyn Fn() -> Vec<AudioDevice>>,
    pub recorder_missing: bool,
}

impl Default for BarSources {
    fn default() -> Self {
        BarSources {
            windows: Rc::new(Vec::new),
            devices: Rc::new(Vec::new),
            recorder_missing: false,
        }
    }
}

pub fn devices_from(runner: Rc<dyn Runner>) -> Rc<dyn Fn() -> Vec<AudioDevice>> {
    Rc::new(move || {
        list_audio_devices(runner.as_ref()).unwrap_or_else(|e| {
            tracing::debug!(error = %e, "no audio devices listed");
            Vec::new()
        })
    })
}

/// Test mode must not ask the user's sound server anything.
pub struct NoAudio;

impl Runner for NoAudio {
    fn run(&self, _: &str, _: &[&str]) -> std::io::Result<String> {
        Err(std::io::Error::other("audio is off in test mode"))
    }
}

/// A refusal is explained by `admit`'s own toast, so only a failed start adds another.
pub fn start_recording(recordings: &Recordings, request: RecordingRequest, cx: &mut App) {
    if recordings.admit(cx).is_err() {
        return;
    }
    let started = recordings.with_request(request, cx);
    cx.spawn(async move |cx| {
        if let Err(e) = started.await
            && !matches!(e, Error::Cancelled)
        {
            tracing::warn!(error = %e, "recording did not start");
            let title = es_app::errors::recording_start_title(&e);
            cx.update(|cx| show_toast(Toast::error(title), cx));
        }
    })
    .detach();
}

pub fn new_bar(w: &Rc<Wiring>, cx: &mut App) -> Entity<CaptureBar> {
    let sources = w.bar_sources.borrow().clone();
    let devices = (sources.devices)();
    let bar = cx.new(|cx| CaptureBar::new((w.outputs)(), devices, sources.recorder_missing, cx));
    let options = bar.read(cx).options().clone();
    let recording = &w.settings.read(cx).config().recording;
    let defaults = RecordingDefaults {
        mic: (!recording.mic.is_empty()).then(|| recording.mic.clone()),
        system_audio: recording.system_audio,
        delay_secs: recording.start_delay,
    };
    options.update(cx, |o, cx| {
        o.set_defaults(defaults, cx);
        o.set_windows((sources.windows)(), cx);
    });
    cx.subscribe(&bar, move |bar, event: &BarEvent, cx| {
        if matches!(event, BarEvent::RefreshWindows) {
            let options = bar.read(cx).options().clone();
            options.update(cx, |o, cx| o.set_windows((sources.windows)(), cx));
        }
    })
    .detach();
    bar
}
