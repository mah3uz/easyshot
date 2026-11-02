use crate::background::{BackgroundSettings, BackgroundStyle, PresetLibrary};
use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureKind {
    Region,
    Fullscreen,
    Window,
    Scroll,
}

/// `Normal` covers region, fullscreen, window, previous region, timed region and scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureVariant {
    Normal,
    RegionCopy,
    RegionSave,
    RegionAnnotate,
    RegionPin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    ShowPreview,
    Copy,
    Save,
    OpenEditor,
    Pin,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    #[serde(with = "crate::time_serde")]
    pub created_at: OffsetDateTime,
    pub kind: CaptureKind,
    pub name: String,
    pub file: PathBuf,
    pub export_path: Option<PathBuf>,
    pub pixel_size: (u32, u32),
    pub has_annotations: bool,
    pub share_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryIndex {
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeckManifest {
    pub id: Uuid,
    pub name: String,
    pub staged: PathBuf,
    pub raw: PathBuf,
    pub thumb: PathBuf,
    pub kind: CaptureKind,
    pub history_id: Option<Uuid>,
    #[serde(with = "crate::time_serde")]
    pub created_at: OffsetDateTime,
}

/// Steps run in this order.
pub fn after_capture_plan(variant: CaptureVariant, cfg: &Config) -> Vec<Step> {
    let rules = &cfg.after_capture.screenshot;
    let mut plan = Vec::new();

    let save = match variant {
        CaptureVariant::Normal => rules.save,
        CaptureVariant::RegionSave => true,
        CaptureVariant::RegionCopy | CaptureVariant::RegionAnnotate | CaptureVariant::RegionPin => {
            false
        }
    };
    if save {
        plan.push(Step::Save);
    }

    let copy = match variant {
        CaptureVariant::RegionCopy => true,
        CaptureVariant::RegionSave => false,
        _ => cfg.general.copy_after_save || rules.copy,
    };
    if copy {
        plan.push(Step::Copy);
    }

    if variant != CaptureVariant::RegionPin && rules.show_preview {
        plan.push(Step::ShowPreview);
    }

    if variant == CaptureVariant::RegionPin || (variant == CaptureVariant::Normal && rules.pin) {
        plan.push(Step::Pin);
    }

    let editor = match variant {
        CaptureVariant::RegionAnnotate => true,
        CaptureVariant::RegionCopy => false,
        _ => cfg.capture.open_editor_after_capture || rules.open_editor,
    };
    if editor {
        plan.push(Step::OpenEditor);
    }
    plan
}

/// The look for a new capture, shared by deck staging and editor open. The active
/// preset wins, then the preset the config names, then the config's own numbers.
pub fn resolve_new_capture_look(cfg: &Config, presets: &PresetLibrary) -> BackgroundSettings {
    let look = &cfg.default_look;
    if let Some(p) = presets.active().or_else(|| presets.find(&look.style)) {
        return p.background.clone();
    }
    BackgroundSettings {
        style: BackgroundStyle::None,
        padding: look.padding,
        corner_radius: look.corner_radius,
        shadow: look.shadow,
        ..BackgroundSettings::default()
    }
}
