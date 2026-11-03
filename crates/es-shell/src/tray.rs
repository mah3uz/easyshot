use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use es_desktop::{RecordingTrayState, Tray, TrayItem, TrayModel};
use es_model::HistoryEntry;
use es_protocol::{CATALOGUE, Route, Target, Trigger};

use crate::strings::fl;

pub const RECENT_PER_KIND: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recent {
    pub name: String,
    pub file: PathBuf,
}

impl From<&HistoryEntry> for Recent {
    fn from(entry: &HistoryEntry) -> Self {
        Recent {
            name: entry.name.clone(),
            file: entry
                .export_path
                .clone()
                .unwrap_or_else(|| entry.file.clone()),
        }
    }
}

/// The tray rows' shortcut labels: the global shortcuts the user saved, as routes.
pub fn shortcuts(saved: &BTreeMap<String, String>) -> Vec<(Route, Trigger)> {
    CATALOGUE
        .iter()
        .filter_map(|e| match &e.target {
            Target::Route(route) => Some((route.clone(), Trigger::parse(saved.get(e.id)?).ok()?)),
            Target::Action(_) => None,
        })
        .collect()
}

const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "webm", "mkv", "gif"];

/// The newest videos in `dir`, newest first.
pub fn recent_recordings(dir: &Path) -> Vec<Recent> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, Recent)> = read
        .flatten()
        .filter_map(|e| {
            let file = e.path();
            let ext = file.extension()?.to_str()?.to_ascii_lowercase();
            if !VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                return None;
            }
            let modified = e.metadata().ok()?.modified().ok()?;
            let name = file.file_name()?.to_string_lossy().into_owned();
            Some((modified, Recent { name, file }))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found
        .into_iter()
        .take(RECENT_PER_KIND)
        .map(|(_, r)| r)
        .collect()
}

/// Everything the tray menu shows, gathered by the caller from the config, history and services.
#[derive(Debug, Clone, Default)]
pub struct TrayInput {
    pub shortcuts: Vec<(Route, Trigger)>,
    /// Newest first, as the history keeps them.
    pub screenshots: Vec<Recent>,
    pub recordings: Vec<Recent>,
    pub has_pins: bool,
    pub recorder_missing: bool,
    pub recording: Option<RecordingTrayState>,
    pub version: String,
    pub update: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIcon {
    Idle,
    /// The clover with a red dot.
    Recording,
    Paused,
}

pub fn icon(input: &TrayInput) -> TrayIcon {
    match input.recording {
        None => TrayIcon::Idle,
        Some(s) if s.paused => TrayIcon::Paused,
        Some(_) => TrayIcon::Recording,
    }
}

pub fn menu(input: &TrayInput) -> TrayModel {
    let mut items = Vec::new();
    if let Some(state) = input.recording {
        items.extend(recording_items(state));
        items.push(separator());
    }
    let action = |id, text: String, route: Route| {
        let shortcut = input
            .shortcuts
            .iter()
            .find(|(r, _)| *r == route)
            .map(|(_, t)| t.label());
        TrayItem {
            shortcut,
            ..item(id, text, Some(route))
        }
    };
    items.extend([
        action("tray-region", fl!("tray-region"), Route::CaptureRegion),
        action("tray-screen", fl!("tray-screen"), Route::CaptureFullscreen),
        action("tray-window", fl!("tray-window"), Route::CaptureWindow),
        match input.recording {
            None if input.recorder_missing => TrayItem {
                enabled: false,
                ..item("tray-record", fl!("tray-record-missing"), None)
            },
            None => action("tray-record", fl!("tray-record"), Route::Record),
            Some(_) => item(
                "tray-record-stop",
                fl!("tray-record-stop"),
                Some(Route::RecordStop),
            ),
        },
        action(
            "tray-scrolling",
            fl!("tray-scrolling"),
            Route::CaptureScroll,
        ),
        action("tray-ocr", fl!("tray-ocr"), Route::Ocr),
        action(
            "tray-pick-color",
            fl!("tray-pick-color"),
            Route::ColorPicker,
        ),
        submenu(
            "tray-recent",
            fl!("tray-recent"),
            vec![
                recent_submenu(
                    "tray-recent-screenshots",
                    fl!("tray-recent-screenshots"),
                    "tray-recent-screenshots-empty",
                    fl!("tray-recent-screenshots-empty"),
                    &input.screenshots,
                ),
                recent_submenu(
                    "tray-recent-recordings",
                    fl!("tray-recent-recordings"),
                    "tray-recent-recordings-empty",
                    fl!("tray-recent-recordings-empty"),
                    &input.recordings,
                ),
            ],
        ),
        item("tray-gallery", fl!("tray-gallery"), Some(Route::Gallery)),
    ]);
    if input.has_pins {
        items.push(item(
            "tray-unpin-all",
            fl!("tray-unpin-all"),
            Some(Route::UnpinAll),
        ));
    }
    items.push(separator());
    items.push(item(
        "tray-settings",
        fl!("tray-settings"),
        Some(Route::Settings(None)),
    ));
    items.push(item("tray-quit", fl!("tray-quit"), Some(Route::Quit)));
    let version = input.version.clone();
    let version = match &input.update {
        None => item("tray-version", fl!("tray-version", version = version), None),
        Some(new) => item(
            "tray-version-update",
            fl!("tray-version-update", version = version, new = new.clone()),
            None,
        ),
    };
    items.push(TrayItem {
        enabled: false,
        ..version
    });
    TrayModel { items }
}

/// Hands the menu and the recording state to the tray service.
pub fn show(tray: &dyn Tray, input: &TrayInput) {
    tray.set_menu(menu(input));
    tray.set_recording(input.recording);
}

pub fn elapsed_text(elapsed: Duration) -> String {
    let s = elapsed.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

fn recording_items(state: RecordingTrayState) -> Vec<TrayItem> {
    let elapsed = elapsed_text(state.elapsed);
    let status = if state.paused {
        item(
            "tray-recording-paused",
            fl!("tray-recording-paused", elapsed = elapsed),
            None,
        )
    } else {
        item(
            "tray-recording",
            fl!("tray-recording", elapsed = elapsed),
            None,
        )
    };
    let pause = if state.paused {
        item(
            "tray-record-resume",
            fl!("tray-record-resume"),
            Some(Route::RecordPause),
        )
    } else {
        item(
            "tray-record-pause",
            fl!("tray-record-pause"),
            Some(Route::RecordPause),
        )
    };
    vec![
        TrayItem {
            enabled: false,
            ..status
        },
        item(
            "tray-record-stop",
            fl!("tray-record-stop"),
            Some(Route::RecordStop),
        ),
        pause,
        item(
            "tray-record-restart",
            fl!("tray-record-restart"),
            Some(Route::RecordRestart),
        ),
        item(
            "tray-record-discard",
            fl!("tray-record-discard"),
            Some(Route::RecordDiscard),
        ),
    ]
}

fn recent_submenu(
    id: &'static str,
    text: String,
    empty_id: &'static str,
    empty: String,
    recent: &[Recent],
) -> TrayItem {
    let children = if recent.is_empty() {
        vec![TrayItem {
            enabled: false,
            ..item(empty_id, empty, None)
        }]
    } else {
        recent
            .iter()
            .take(RECENT_PER_KIND)
            .map(|r| {
                item(
                    "tray-recent-item",
                    r.name.clone(),
                    Some(Route::Open(r.file.clone())),
                )
            })
            .collect()
    };
    submenu(id, text, children)
}

fn item(label_id: &'static str, label: String, route: Option<Route>) -> TrayItem {
    TrayItem {
        label_id,
        label: Some(label),
        shortcut: None,
        route,
        children: Vec::new(),
        enabled: true,
        separator: false,
    }
}

fn submenu(label_id: &'static str, label: String, children: Vec<TrayItem>) -> TrayItem {
    TrayItem {
        children,
        ..item(label_id, label, None)
    }
}

fn separator() -> TrayItem {
    TrayItem {
        separator: true,
        ..item("", String::new(), None)
    }
}
