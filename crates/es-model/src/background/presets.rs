use super::{BackgroundSettings, BorderSettings, ProgressiveBlurSettings};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_PRESET_NAME_CHARS: usize = 48;
const FALLBACK_NAME: &str = "Untitled Preset";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundPreset {
    pub id: Uuid,
    pub name: String,
    pub background: BackgroundSettings,
}

impl Default for BackgroundPreset {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: FALLBACK_NAME.into(),
            background: BackgroundSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetLibrary {
    pub presets: Vec<BackgroundPreset>,
    /// The preset applied to new screenshots.
    pub active_preset_id: Option<Uuid>,
}

fn collapse_name(raw: &str) -> String {
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(MAX_PRESET_NAME_CHARS).collect();
    cut.trim_end().to_owned()
}

impl PresetLibrary {
    /// Whitespace collapsed, at most 48 characters, never empty.
    pub fn sanitize_name(raw: &str) -> String {
        let name = collapse_name(raw);
        if name.is_empty() {
            FALLBACK_NAME.into()
        } else {
            name
        }
    }

    fn name_taken(&self, name: &str, except: Option<Uuid>) -> bool {
        let lower = name.to_lowercase();
        self.presets
            .iter()
            .any(|p| Some(p.id) != except && p.name.to_lowercase() == lower)
    }

    /// Case-insensitive unique name; a clash gets " 2", " 3", and so on.
    pub fn unique_name(&self, raw: &str, except: Option<Uuid>) -> String {
        let base = Self::sanitize_name(raw);
        if !self.name_taken(&base, except) {
            return base;
        }
        (2..)
            .map(|n| {
                let suffix = format!(" {n}");
                let room = MAX_PRESET_NAME_CHARS - suffix.chars().count();
                let head: String = base.chars().take(room).collect();
                format!("{}{suffix}", head.trim_end())
            })
            .find(|candidate| !self.name_taken(candidate, except))
            .unwrap_or(base)
    }

    /// Saving a preset makes it the default for new screenshots.
    pub fn add(&mut self, name: &str, background: BackgroundSettings) -> Uuid {
        let id = Uuid::new_v4();
        let name = self.unique_name(name, None);
        self.presets.push(BackgroundPreset {
            id,
            name,
            background,
        });
        self.active_preset_id = Some(id);
        id
    }

    pub fn remove(&mut self, id: Uuid) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.id != id);
        if self.active_preset_id == Some(id) {
            self.active_preset_id = None;
        }
        self.presets.len() != before
    }

    pub fn get(&self, id: Uuid) -> Option<&BackgroundPreset> {
        self.presets.iter().find(|p| p.id == id)
    }

    pub fn active(&self) -> Option<&BackgroundPreset> {
        self.active_preset_id.and_then(|id| self.get(id))
    }

    /// Looks a preset up by id, then by name ignoring case.
    pub fn find(&self, key: &str) -> Option<&BackgroundPreset> {
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        if let Ok(id) = Uuid::parse_str(key)
            && let Some(p) = self.get(id)
        {
            return Some(p);
        }
        let lower = key.to_lowercase();
        self.presets.iter().find(|p| p.name.to_lowercase() == lower)
    }
}

impl BackgroundSettings {
    /// Equal looks, ignoring sub-settings that are switched off.
    pub fn same_look_as(&self, other: &BackgroundSettings) -> bool {
        fn normalised(b: &BackgroundSettings) -> BackgroundSettings {
            let mut b = b.clone();
            if !b.progressive_blur.is_enabled {
                b.progressive_blur = ProgressiveBlurSettings::default();
            }
            if !b.border.is_enabled {
                b.border = BorderSettings::default();
            }
            b
        }
        normalised(self) == normalised(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::BackgroundStyle;

    #[test]
    fn names_are_collapsed_capped_and_never_empty() {
        assert_eq!(PresetLibrary::sanitize_name("  my \t  look \n"), "my look");
        assert_eq!(PresetLibrary::sanitize_name("   "), "Untitled Preset");
        let long = "x".repeat(80);
        assert_eq!(PresetLibrary::sanitize_name(&long).chars().count(), 48);
    }

    #[test]
    fn clashing_names_get_a_numeric_suffix_ignoring_case() {
        let mut lib = PresetLibrary::default();
        lib.add("Ocean", BackgroundSettings::default());
        lib.add("ocean", BackgroundSettings::default());
        lib.add("OCEAN", BackgroundSettings::default());
        let names: Vec<_> = lib.presets.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Ocean", "ocean 2", "OCEAN 3"]);
    }

    #[test]
    fn a_suffixed_name_still_fits_the_cap() {
        let mut lib = PresetLibrary::default();
        let long = "y".repeat(48);
        lib.add(&long, BackgroundSettings::default());
        let second = lib.unique_name(&long, None);
        assert_eq!(second.chars().count(), 48);
        assert!(second.ends_with(" 2"));
    }

    #[test]
    fn renaming_to_its_own_name_is_not_a_clash() {
        let mut lib = PresetLibrary::default();
        let id = lib.add("Dusk", BackgroundSettings::default());
        assert_eq!(lib.unique_name("dusk", Some(id)), "dusk");
    }

    #[test]
    fn saving_activates_and_removing_the_active_preset_clears_it() {
        let mut lib = PresetLibrary::default();
        let a = lib.add("A", BackgroundSettings::default());
        let b = lib.add("B", BackgroundSettings::default());
        assert_eq!(lib.active_preset_id, Some(b));
        assert!(lib.remove(a));
        assert_eq!(lib.active_preset_id, Some(b));
        assert!(lib.remove(b));
        assert!(lib.active().is_none());
        assert!(!lib.remove(b));
    }

    #[test]
    fn find_accepts_an_id_or_a_name() {
        let mut lib = PresetLibrary::default();
        let id = lib.add("Sunset Pro", BackgroundSettings::default());
        assert_eq!(lib.find(&id.to_string()).map(|p| p.id), Some(id));
        assert_eq!(lib.find(" sunset pro ").map(|p| p.id), Some(id));
        assert!(lib.find("").is_none());
        assert!(lib.find("missing").is_none());
    }

    #[test]
    fn disabled_effects_do_not_make_two_looks_differ() {
        let a = BackgroundSettings::default();
        let mut b = a.clone();
        b.progressive_blur.strength = 40.0;
        b.border.thickness = 0.1;
        assert!(a.same_look_as(&b));
        b.border.is_enabled = true;
        assert!(!a.same_look_as(&b));
        let mut c = a.clone();
        c.style = BackgroundStyle::Wallpaper {
            path: "/w.png".into(),
        };
        assert!(!a.same_look_as(&c));
    }
}
