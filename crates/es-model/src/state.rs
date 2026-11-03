use crate::AnnotationDocument;
use crate::time_serde;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

pub const MAX_RECENT_WALLPAPERS: usize = 24;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastRegion {
    pub output: String,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// What `capture previous` repeats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LastTarget {
    Region(LastRegion),
    /// App and title name the window once it has closed.
    Window {
        id: String,
        app: String,
        title: String,
    },
    Output(String),
}

/// App-written state, never edited by hand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Where the region selector starts.
    pub last_region: Option<LastRegion>,
    pub last_target: Option<LastTarget>,
    /// Next value for a `{counter}` in the file name template.
    pub counter: u32,
    pub onboarding_version: u32,
    /// Inspector section id to expanded.
    pub inspector_sections: BTreeMap<String, bool>,
    /// Newest first, at most `MAX_RECENT_WALLPAPERS`.
    pub recent_wallpapers: Vec<PathBuf>,
    /// Custom deck layout: hover slot to tool. Empty means the layout preset in the config.
    pub deck_layout: BTreeMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            last_region: None,
            last_target: None,
            counter: 1,
            onboarding_version: 0,
            inspector_sections: BTreeMap::new(),
            recent_wallpapers: Vec::new(),
            deck_layout: BTreeMap::new(),
        }
    }
}

impl State {
    pub fn push_recent_wallpaper(&mut self, path: PathBuf) {
        self.recent_wallpapers.retain(|p| p != &path);
        self.recent_wallpapers.insert(0, path);
        self.recent_wallpapers.truncate(MAX_RECENT_WALLPAPERS);
    }
}

/// One editor session's unsaved edits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recovery {
    pub session: Uuid,
    #[serde(with = "time_serde")]
    pub saved_at: OffsetDateTime,
    /// The image being edited, when it came from a file.
    pub source: Option<PathBuf>,
    pub base_image: PathBuf,
    /// Changes whenever crop, rotate or flip replaces the base image.
    pub base_revision: Uuid,
    pub document: AnnotationDocument,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            session: Uuid::new_v4(),
            saved_at: OffsetDateTime::UNIX_EPOCH,
            source: None,
            base_image: PathBuf::new(),
            base_revision: Uuid::nil(),
            document: AnnotationDocument::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_wallpapers_are_newest_first_unique_and_capped() {
        let mut s = State::default();
        for i in 0..30 {
            s.push_recent_wallpaper(format!("/w/{i}.png").into());
        }
        assert_eq!(s.recent_wallpapers.len(), 24);
        assert_eq!(s.recent_wallpapers[0], PathBuf::from("/w/29.png"));
        s.push_recent_wallpaper("/w/10.png".into());
        assert_eq!(s.recent_wallpapers[0], PathBuf::from("/w/10.png"));
        assert_eq!(
            s.recent_wallpapers
                .iter()
                .filter(|p| p.ends_with("10.png"))
                .count(),
            1
        );
        assert_eq!(s.recent_wallpapers.len(), 24);
    }

    #[test]
    fn fresh_state_starts_the_counter_at_one() {
        let s: State = serde_json::from_str("{}").unwrap();
        assert_eq!(s, State::default());
        assert_eq!((s.counter, s.onboarding_version), (1, 0));
    }

    #[test]
    fn recovery_carries_the_whole_document() {
        use crate::shape::{AnnoShape, NumberedProps, ShapeKind};
        let mut r = Recovery::default();
        r.document.add_shape(AnnoShape::new(
            1.0,
            2.0,
            ShapeKind::Numbered(NumberedProps::default()),
        ));
        let back: Recovery = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }
}
