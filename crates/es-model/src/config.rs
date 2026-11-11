use crate::agent::AgentCfg;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub config_version: u32,
    pub general: General,
    pub capture: CaptureCfg,
    pub overlay: Overlay,
    pub after_capture: AfterCapture,
    pub recording: Recording,
    pub default_look: DefaultLook,
    pub editor: Editor,
    pub ocr: Ocr,
    pub gallery: GalleryCfg,
    pub agent: AgentCfg,
    pub share: ShareCfg,
    /// Editor-scope overrides: catalogue id to trigger.
    pub shortcuts: BTreeMap<String, String>,
    /// Global shortcuts the compositor binds to `easyshot <command>`: catalogue id to trigger.
    pub global_shortcuts: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: 1,
            general: General::default(),
            capture: CaptureCfg::default(),
            overlay: Overlay::default(),
            after_capture: AfterCapture::default(),
            recording: Recording::default(),
            default_look: DefaultLook::default(),
            editor: Editor::default(),
            ocr: Ocr::default(),
            gallery: GalleryCfg::default(),
            agent: AgentCfg::default(),
            share: ShareCfg::default(),
            shortcuts: BTreeMap::new(),
            global_shortcuts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub show_capture_bar_at_launch: bool,
    pub theme: String,
    /// None means the XDG Pictures folder's `Screenshots` directory, resolved at runtime.
    pub save_dir: Option<PathBuf>,
    pub auto_save: bool,
    pub file_name_template: String,
    pub copy_after_save: bool,
    pub play_sound: bool,
    pub export_format: String,
    pub export_quality: f64,
    pub history_limit: u32,
    pub editor_opens_fullscreen: bool,
    pub low_res_preview: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            show_capture_bar_at_launch: false,
            theme: "system".into(),
            save_dir: None,
            auto_save: true,
            file_name_template: "easyshot_{date}-{time}".into(),
            copy_after_save: true,
            play_sound: true,
            export_format: "png".into(),
            export_quality: 0.9,
            history_limit: 100,
            editor_opens_fullscreen: false,
            low_res_preview: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureCfg {
    pub self_timer: u32,
    pub capture_on_release: bool,
    pub preselect_previous: bool,
    pub selector_follows_pointer: bool,
    pub pinned_output: String,
    pub open_editor_after_capture: bool,
    pub keep_in_deck_until_saved: bool,
}

impl Default for CaptureCfg {
    fn default() -> Self {
        Self {
            self_timer: 0,
            capture_on_release: false,
            preselect_previous: true,
            selector_follows_pointer: true,
            pinned_output: String::new(),
            open_editor_after_capture: false,
            keep_in_deck_until_saved: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Overlay {
    pub layout: String,
    pub position: String,
    pub card_size: String,
    pub always_show_actions: bool,
    pub edge_margin: u32,
    /// Seconds; 16 means never.
    pub dismiss_delay: u32,
    pub actions: CardActions,
}

impl Default for Overlay {
    fn default() -> Self {
        Self {
            layout: "standard".into(),
            position: "bottom-right".into(),
            card_size: "small".into(),
            always_show_actions: false,
            edge_margin: 20,
            dismiss_delay: 5,
            actions: CardActions::default(),
        }
    }
}

/// The actions a deck card offers besides Dismiss; Save stands for Show in folder once auto-save wrote the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardActions {
    pub copy: bool,
    pub save: bool,
    pub edit: bool,
    pub pin: bool,
    pub share: bool,
    pub preview: bool,
    pub delete: bool,
}

impl Default for CardActions {
    fn default() -> Self {
        Self {
            copy: true,
            save: true,
            edit: true,
            pin: true,
            share: true,
            preview: false,
            delete: false,
        }
    }
}

impl CardActions {
    pub fn any(&self) -> bool {
        [
            self.copy,
            self.save,
            self.edit,
            self.pin,
            self.share,
            self.preview,
            self.delete,
        ]
        .contains(&true)
    }

    /// A set with nothing on cannot be chosen in Settings; read from a file, it means the defaults.
    pub fn or_default(self) -> Self {
        if self.any() { self } else { Self::default() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AfterCapture {
    pub screenshot: AfterCaptureScreenshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AfterCaptureScreenshot {
    pub show_preview: bool,
    pub copy: bool,
    pub open_editor: bool,
    pub pin: bool,
}

impl Default for AfterCaptureScreenshot {
    fn default() -> Self {
        Self {
            show_preview: true,
            copy: false,
            open_editor: false,
            pin: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recording {
    pub save: bool,
    pub mic: String,
    pub system_audio: bool,
    pub keystrokes: bool,
    pub start_delay: u32,
    pub open_editor_after: bool,
    pub whole_screen_bar: bool,
    pub fps: u32,
    pub codec: String,
    pub container: String,
    pub quality: String,
    /// "auto" or a provider name: gsr-native, gsr-portal, wf-recorder, portal-pipewire.
    pub recorder: String,
    pub export_format: String,
    pub export_size: String,
    pub export_quality: String,
    pub export_look: String,
    pub export_brightness: f64,
    pub export_system_volume: f64,
    pub export_mic_volume: f64,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            save: false,
            mic: String::new(),
            system_audio: true,
            keystrokes: false,
            start_delay: 0,
            open_editor_after: false,
            whole_screen_bar: true,
            fps: 60,
            codec: "h264".into(),
            container: "mkv".into(),
            quality: "very_high".into(),
            recorder: "auto".into(),
            export_format: "mp4".into(),
            export_size: "original".into(),
            export_quality: "balanced".into(),
            export_look: "natural".into(),
            export_brightness: 0.,
            export_system_volume: 0.5,
            export_mic_volume: 1.,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultLook {
    /// Preset id or name; empty means none.
    pub style: String,
    pub padding: f64,
    pub corner_radius: f64,
    pub shadow: f64,
}

impl Default for DefaultLook {
    fn default() -> Self {
        Self {
            style: String::new(),
            padding: 0.08,
            corner_radius: 0.018,
            shadow: 0.36,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Editor {
    pub nudge_step: u32,
    pub nudge_step_shift: u32,
    pub zoom_anchor: String,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            nudge_step: 1,
            nudge_step_shift: 10,
            zoom_anchor: "pointer".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ocr {
    pub languages: Vec<String>,
}

impl Default for Ocr {
    fn default() -> Self {
        Self {
            languages: vec!["eng".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GalleryCfg {
    /// The side of an icon-view preview, in pixels.
    pub item_size: u32,
}

impl Default for GalleryCfg {
    fn default() -> Self {
        Self { item_size: 96 }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareMode {
    #[default]
    Free,
    Bucket,
}

pub const SHARE_DAYS: [u8; 3] = [1, 3, 7];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShareCfg {
    pub days: u8,
    pub bucket_endpoint: String,
    pub bucket_name: String,
    pub bucket_region: String,
    pub bucket_public_base_url: String,
    pub bucket_prefix: String,
}

impl Default for ShareCfg {
    fn default() -> Self {
        Self {
            days: 7,
            bucket_endpoint: String::new(),
            bucket_name: String::new(),
            bucket_region: String::new(),
            bucket_public_base_url: String::new(),
            bucket_prefix: String::new(),
        }
    }
}

impl ShareCfg {
    /// The bucket once its details are filled in, else the free service.
    pub fn mode(&self) -> ShareMode {
        let filled = |v: &str| !v.trim().is_empty();
        if filled(&self.bucket_endpoint)
            && filled(&self.bucket_name)
            && filled(&self.bucket_public_base_url)
        {
            ShareMode::Bucket
        } else {
            ShareMode::Free
        }
    }

    /// A hand-edited lifetime the service does not offer reads as the default.
    pub fn days(&self) -> u8 {
        if SHARE_DAYS.contains(&self.days) {
            self.days
        } else {
            7
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_without_a_share_section_shares_to_free() {
        let c: Config = toml::from_str("[general]\ntheme = \"dark\"\n").unwrap();
        assert_eq!(c.share, ShareCfg::default());
        assert_eq!(c.share.mode(), ShareMode::Free);
        assert_eq!(c.share.days(), 7);
    }

    #[test]
    fn a_share_lifetime_the_service_does_not_offer_falls_back_to_seven_days() {
        for (days, want) in [(1, 1), (3, 3), (7, 7), (0, 7), (2, 7), (30, 7)] {
            let c: Config = toml::from_str(&format!("[share]\ndays = {days}\n")).unwrap();
            assert_eq!(c.share.days(), want, "days = {days}");
        }
    }

    #[test]
    fn the_destination_is_the_bucket_only_once_its_details_are_filled() {
        let mut c = ShareCfg::default();
        assert_eq!(c.mode(), ShareMode::Free);
        c.bucket_endpoint = "https://s3.example".into();
        c.bucket_name = "b".into();
        assert_eq!(
            c.mode(),
            ShareMode::Free,
            "a half-filled bucket is not chosen"
        );
        c.bucket_public_base_url = "  ".into();
        assert_eq!(c.mode(), ShareMode::Free, "blanks do not count");
        c.bucket_public_base_url = "https://cdn.example".into();
        assert_eq!(c.mode(), ShareMode::Bucket);
    }

    #[test]
    fn the_removed_target_and_consent_keys_in_an_old_file_are_ignored() {
        let c: Config =
            toml::from_str("[share]\ntarget = \"bucket\"\nconsented = true\nbucket_name = \"b\"\n")
                .unwrap();
        assert_eq!(c.share.mode(), ShareMode::Free);
        assert_eq!(c.share.bucket_name, "b");
    }

    #[test]
    fn empty_toml_gives_the_documented_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.config_version, 1);
        assert!(c.general.auto_save);
        assert!(
            !c.general.show_capture_bar_at_launch,
            "the daemon starts at login, which must stay in the tray"
        );
        assert_eq!(c.general.file_name_template, "easyshot_{date}-{time}");
        assert_eq!(c.general.history_limit, 100);
        assert_eq!(c.overlay.dismiss_delay, 5);
        assert!(c.after_capture.screenshot.show_preview);
        assert!(!c.after_capture.screenshot.copy);
        assert_eq!((c.recording.fps, c.recording.codec.as_str()), (60, "h264"));
        assert_eq!(c.default_look.padding, 0.08);
        assert_eq!(c.editor.nudge_step_shift, 10);
        assert_eq!(c.ocr.languages, vec!["eng"]);
        assert_eq!(c.gallery.item_size, 96);
    }

    #[test]
    fn partial_section_keeps_other_defaults_and_unknown_keys_are_ignored() {
        let c: Config = toml::from_str(
            "future_key = 1\n[general]\ntheme = \"dark\"\nnew_thing = true\n[future_section]\nx = 1\n",
        )
        .unwrap();
        assert_eq!(c.general.theme, "dark");
        assert!(c.general.auto_save);
        assert_eq!(c.capture, CaptureCfg::default());
    }

    #[test]
    fn serialised_defaults_parse_back_identically() {
        let s = toml::to_string(&Config::default()).unwrap();
        assert_eq!(toml::from_str::<Config>(&s).unwrap(), Config::default());
    }

    #[test]
    fn v1_fixture_loads_and_equals_defaults() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/formats/config/v1.toml"
        ));
        let mut expected = Config::default();
        // Files written before the defaults changed store the old values, and a stored choice is kept.
        expected.general.show_capture_bar_at_launch = true;
        expected.recording.system_audio = false;
        assert_eq!(toml::from_str::<Config>(src).unwrap(), expected);
    }

    #[test]
    fn a_card_action_left_out_of_the_file_keeps_its_default() {
        let c: Config = toml::from_str("[overlay.actions]\ndelete = true\ncopy = false\n").unwrap();
        let a = c.overlay.actions;
        assert!(a.delete && !a.copy, "the chosen ones are read");
        assert!(a.save && a.edit && a.pin && a.share && !a.preview);
    }

    #[test]
    fn a_hand_edited_set_with_nothing_on_reads_as_the_defaults() {
        let none = CardActions {
            copy: false,
            save: false,
            edit: false,
            pin: false,
            share: false,
            preview: false,
            delete: false,
        };
        assert!(!none.any());
        assert_eq!(none.or_default(), CardActions::default());
        let one = CardActions {
            preview: true,
            ..none
        };
        assert_eq!(one.or_default(), one, "one action on is a valid choice");
    }
}
