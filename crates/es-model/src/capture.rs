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
pub struct BackgroundSettings {
    pub style: String,
    pub padding: f64,
    pub corner_radius: f64,
    pub shadow: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetLibrary {
    pub presets: Vec<BackgroundSettings>,
}

mod rfc3339 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    pub fn serialize<S: Serializer>(t: &OffsetDateTime, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&t.format(&Rfc3339).map_err(serde::ser::Error::custom)?)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<OffsetDateTime, D::Error> {
        let s = String::deserialize(d)?;
        OffsetDateTime::parse(&s, &Rfc3339).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    #[serde(with = "rfc3339")]
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
    #[serde(with = "rfc3339")]
    pub created_at: OffsetDateTime,
}

pub fn after_capture_plan(_variant: CaptureVariant, _cfg: &Config) -> Vec<Step> {
    todo!()
}

pub fn resolve_new_capture_look(_cfg: &Config, _presets: &PresetLibrary) -> BackgroundSettings {
    todo!()
}
