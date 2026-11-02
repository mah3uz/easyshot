use super::windows::Wiring;
use es_model::platform::WindowInfo;
use es_overlays::record::bar::{BarEvent, CaptureBar};
use es_record::AudioDevice;
use es_record::audio::{Runner, list_audio_devices};
use gpui::{App, AppContext as _, Entity};
use std::rc::Rc;

/// What the capture bar lists: windows to record and microphones to record with.
#[derive(Clone)]
pub struct BarSources {
    pub windows: Rc<dyn Fn() -> Vec<WindowInfo>>,
    pub devices: Rc<dyn Fn() -> Vec<AudioDevice>>,
}

impl Default for BarSources {
    fn default() -> Self {
        BarSources {
            windows: Rc::new(Vec::new),
            devices: Rc::new(Vec::new),
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

pub fn new_bar(w: &Rc<Wiring>, cx: &mut App) -> Entity<CaptureBar> {
    let sources = w.bar_sources.borrow().clone();
    let devices = (sources.devices)();
    let bar = cx.new(|cx| CaptureBar::new((w.outputs)(), devices, cx));
    let options = bar.read(cx).options().clone();
    options.update(cx, |o, cx| o.set_windows((sources.windows)(), cx));
    cx.subscribe(&bar, move |bar, event: &BarEvent, cx| {
        if matches!(event, BarEvent::RefreshWindows) {
            let options = bar.read(cx).options().clone();
            options.update(cx, |o, cx| o.set_windows((sources.windows)(), cx));
        }
    })
    .detach();
    bar
}
