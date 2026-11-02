mod rows;
mod view;

use es_backend::Caps;
use es_model::State;
use es_store::paths::Paths;

pub use rows::check_caption;
pub use view::{HEIGHT, Onboarding, OnboardingEvent, OnboardingInput, Step, WIDTH};

/// The `onboarding_version` a finished or skipped introduction writes.
pub const CURRENT_VERSION: u32 = 1;

/// Run before anything else writes `state.json`. True only for a new install: a profile with
/// a config or a history but no state file is an existing user and is marked seen.
pub fn prepare_for_launch(paths: &Paths) -> es_store::error::Result<bool> {
    if es_store::json::JsonFile::<State>::open_existing(
        &paths.state_file(),
        es_model::FormatKind::State,
    )?
    .is_some()
    {
        return Ok(false);
    }
    let existing = paths.config_file().exists() || paths.history_index().exists();
    let mut file = es_store::state::open(paths)?;
    file.value.onboarding_version = if existing { CURRENT_VERSION } else { 0 };
    file.save()?;
    Ok(!existing)
}

pub fn should_present(state: &State) -> bool {
    state.onboarding_version == 0
}

/// Skipping, closing and finishing all count as seen, so an optional introduction never nags.
pub fn mark_seen(state: &mut State) {
    state.onboarding_version = state.onboarding_version.max(CURRENT_VERSION);
}

pub fn complete(paths: &Paths) -> es_store::error::Result<()> {
    let mut file = es_store::state::open(paths)?;
    mark_seen(&mut file.value);
    file.save()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Capture,
    LayerShell,
    Clipboard,
    Tray,
    Recorder,
    Ocr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckRow {
    pub check: Check,
    pub ready: bool,
    pub required: bool,
}

pub const CHECKS: [(Check, Caps, bool); 6] = [
    (Check::Capture, Caps::CAPTURE_OUTPUT, true),
    (Check::LayerShell, Caps::LAYER_SHELL, true),
    (Check::Clipboard, Caps::CLIPBOARD_NO_FOCUS, false),
    (Check::Tray, Caps::TRAY, false),
    (Check::Recorder, Caps::RECORDER, false),
    (Check::Ocr, Caps::OCR, false),
];

pub fn checklist(caps: Caps) -> Vec<CheckRow> {
    CHECKS
        .iter()
        .map(|&(check, cap, required)| CheckRow {
            check,
            ready: caps.contains(cap),
            required,
        })
        .collect()
}

/// What the shortcut part of the checklist shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutSetup {
    /// Lines for the user to paste into the compositor config.
    Snippet(String),
    /// The desktop's shortcut portal asks the user itself.
    Portal,
}

impl ShortcutSetup {
    pub fn for_caps(caps: Caps, snippet: Option<String>) -> Option<Self> {
        if caps.contains(Caps::PORTAL_SHORTCUTS) {
            return Some(ShortcutSetup::Portal);
        }
        snippet
            .filter(|s| !s.trim().is_empty())
            .map(ShortcutSetup::Snippet)
    }
}
