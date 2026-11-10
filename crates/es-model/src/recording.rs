use crate::time_serde;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

/// `capture.json`; written when a recording finishes, so its absence marks a crash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordingCapture {
    #[serde(with = "time_serde")]
    pub created_at: OffsetDateTime,
    pub duration_secs: f64,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub pixel_scale: f64,
    pub source_output: String,
    pub video_file: String,
    pub fps: u32,
    pub codec: String,
    pub container: String,
    pub recorder: String,
    pub includes_system_audio: bool,
    pub includes_microphone: bool,
    /// The sound tracks of the video file, in file order.
    pub audio_tracks: Vec<AudioTrack>,
    pub pointer_synthesized: bool,
    pub press_effects_baked: bool,
}

impl Default for RecordingCapture {
    fn default() -> Self {
        Self {
            created_at: OffsetDateTime::UNIX_EPOCH,
            duration_secs: 0.0,
            pixel_width: 0,
            pixel_height: 0,
            pixel_scale: 1.0,
            source_output: String::new(),
            video_file: "screen.mkv".into(),
            fps: 60,
            codec: "h264".into(),
            container: "mkv".into(),
            recorder: String::new(),
            includes_system_audio: false,
            includes_microphone: false,
            audio_tracks: Vec::new(),
            pointer_synthesized: false,
            press_effects_baked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioTrack {
    System,
    Mic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TravelKind {
    #[default]
    Move,
    Drag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressPhase {
    #[default]
    Down,
    Up,
}

/// `time` is seconds on the movie timeline with pauses removed; `x` and `y` are
/// normalised to the capture rectangle, 0..=1 from the top left.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PointerSample {
    pub time: f64,
    pub x: f64,
    pub y: f64,
    pub kind: TravelKind,
}

/// `x` and `y` are absent when the pointer position could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PressSample {
    pub time: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    pub button: u32,
    pub phase: PressPhase,
}

/// Only modifier chords and special keys are ever stored, never plain typing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeystrokeSample {
    pub time: f64,
    pub modifiers: Vec<String>,
    pub key: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordingInput {
    pub travel: Vec<PointerSample>,
    pub presses: Vec<PressSample>,
    pub keystrokes: Vec<KeystrokeSample>,
    pub is_sanitized: bool,
}

/// Whether recordings can read the keyboards and mice for `input.json`. `GroupPending` is
/// `Denied` for a user already listed in the `input` group whose session predates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAccess {
    Granted,
    Denied,
    GroupPending,
    NotBuilt,
}

/// `project.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordingProject {
    pub display_name: String,
    #[serde(with = "time_serde")]
    pub saved_at: OffsetDateTime,
    #[serde(with = "time_serde::option")]
    pub last_opened_at: Option<OffsetDateTime>,
    /// The movie inside the project folder.
    pub file_name: String,
    /// Where the deliverable was last exported.
    pub export_path: Option<PathBuf>,
}

impl Default for RecordingProject {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            saved_at: OffsetDateTime::UNIX_EPOCH,
            last_opened_at: None,
            file_name: "screen.mkv".into(),
            export_path: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_synthesised_recovery_manifest_needs_only_the_known_fields() {
        let c: RecordingCapture =
            serde_json::from_str(r#"{"duration_secs":3.5,"pixel_width":1920,"pixel_height":1080}"#)
                .unwrap();
        assert_eq!(
            (c.duration_secs, c.pixel_width, c.pixel_height),
            (3.5, 1920, 1080)
        );
        assert_eq!(
            (c.video_file.as_str(), c.fps, c.pixel_scale),
            ("screen.mkv", 60, 1.0)
        );
        assert!(!c.pointer_synthesized && !c.press_effects_baked);
    }

    #[test]
    fn input_streams_default_empty_and_unsanitised() {
        let i: RecordingInput = serde_json::from_str("{}").unwrap();
        assert!(i.travel.is_empty() && i.presses.is_empty() && i.keystrokes.is_empty());
        assert!(!i.is_sanitized);
    }

    #[test]
    fn project_dates_are_rfc3339_and_the_open_date_is_optional() {
        let p: RecordingProject = serde_json::from_str(
            r#"{"display_name":"demo","saved_at":"2026-03-04T05:06:07Z","file_name":"screen.mkv"}"#,
        )
        .unwrap();
        assert_eq!(p.saved_at.unix_timestamp(), 1_772_600_767);
        assert_eq!(p.last_opened_at, None);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["saved_at"], "2026-03-04T05:06:07Z");
        assert_eq!(v["last_opened_at"], serde_json::Value::Null);
    }
}
