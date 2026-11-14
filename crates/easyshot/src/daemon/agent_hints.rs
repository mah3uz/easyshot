use crate::strings::fl;
use es_model::platform::WindowInfo;

const LISTED: usize = 5;

pub(super) fn access_off() -> String {
    fl!("agent-hint-access-off")
}

pub(super) fn access_denied() -> String {
    fl!("agent-hint-access-denied")
}

pub(super) fn change_cancelled() -> String {
    fl!("agent-hint-change-cancelled")
}

pub(super) fn outside_scope() -> String {
    fl!("agent-hint-outside-scope")
}

pub(super) fn capture_busy() -> String {
    fl!("agent-hint-capture-busy")
}

pub(super) fn recording_busy() -> String {
    fl!("agent-hint-recording-busy")
}

pub(super) fn tesseract_missing() -> String {
    fl!("agent-hint-tesseract-missing")
}

pub(super) fn no_recorder() -> String {
    fl!("agent-hint-no-recorder")
}

pub(super) fn window_off_screen() -> String {
    fl!("agent-hint-window-off-screen")
}

pub(super) fn ffmpeg_missing() -> String {
    fl!("agent-hint-ffmpeg-missing")
}

pub(super) fn area_off_monitor() -> String {
    fl!("agent-hint-area-off-monitor")
}

pub(super) fn audio_device_missing(device: Option<&str>) -> String {
    match device.filter(|d| !d.is_empty()) {
        Some(device) => fl!("agent-hint-no-audio-device", device = device),
        None => fl!("agent-hint-no-audio-device-default"),
    }
}

pub(super) fn no_output(query: &str, outputs: &[String]) -> String {
    fl!(
        "agent-hint-no-output",
        query = query,
        outputs = listed(outputs.iter().map(String::as_str))
    )
}

pub(super) fn no_window(query: &str, windows: &[WindowInfo]) -> String {
    if windows.is_empty() {
        return fl!("agent-hint-no-window-none", query = query);
    }
    fl!(
        "agent-hint-no-window",
        query = query,
        windows = names(windows.iter())
    )
}

pub(super) fn ambiguous_window(query: &str, found: &[&WindowInfo]) -> String {
    fl!(
        "agent-hint-window-ambiguous",
        query = query,
        windows = names(found.iter().copied())
    )
}

fn names<'a>(windows: impl Iterator<Item = &'a WindowInfo>) -> String {
    let clean = |s: &str| s.replace(|c: char| c.is_control(), " ");
    let named: Vec<String> = windows
        .map(|w| match (w.title.is_empty(), w.app_id.is_empty()) {
            (false, false) => format!("{} ({})", clean(&w.title), clean(&w.app_id)),
            (false, true) => clean(&w.title),
            _ => clean(&w.app_id),
        })
        .collect();
    listed(named.iter().map(String::as_str))
}

fn listed<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let all: Vec<&str> = items.collect();
    let mut text = all
        .iter()
        .take(LISTED)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if all.len() > LISTED {
        text.push_str(", …");
    }
    text
}
