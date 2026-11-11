use es_model::{HistoryEntry, ShareRecord};
use es_store::Paths;
use es_store::media::{self, MediaFile};
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

/// The sidebar's two halves: files on this computer, and every link you shared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Section {
    #[default]
    Local,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    All,
    Screenshots,
    Videos,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::All, Category::Screenshots, Category::Videos];

    pub fn admits(self, kind: MediaKind) -> bool {
        match self {
            Category::All => true,
            Category::Screenshots => kind == MediaKind::Screenshot,
            Category::Videos => kind == MediaKind::Video,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaKind {
    Screenshot,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    Name,
    Kind,
    #[default]
    NewestFirst,
    OldestFirst,
}

impl SortOrder {
    pub const ALL: [SortOrder; 4] = [
        SortOrder::Name,
        SortOrder::Kind,
        SortOrder::NewestFirst,
        SortOrder::OldestFirst,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Icons,
    List,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub file: PathBuf,
    pub created_at: OffsetDateTime,
    pub kind: MediaKind,
    pub duration_secs: Option<f64>,
    /// What moves to the Trash with the item: the recording folder of an unsaved video.
    pub project: Option<PathBuf>,
    pub share: Option<ShareRecord>,
    /// False for a shared item whose file is gone.
    pub local: bool,
}

impl Item {
    /// The name is the file stem, so the extension is drawn apart from it.
    pub fn extension_suffix(&self) -> String {
        self.file
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default()
    }
}

impl From<MediaFile> for Item {
    fn from(m: MediaFile) -> Self {
        Self {
            id: m.id,
            name: m.name,
            file: m.file,
            created_at: m.created_at,
            kind: match m.kind {
                media::MediaKind::Image => MediaKind::Screenshot,
                media::MediaKind::Video => MediaKind::Video,
            },
            duration_secs: m.duration_secs,
            project: m.project,
            share: None,
            local: true,
        }
    }
}

/// The history's images and every video, newest first; a capture known only by its cloud link has no local view.
pub fn local(paths: &Paths, entries: &[HistoryEntry]) -> Vec<Item> {
    media::list(paths, entries)
        .into_iter()
        .filter(|m| m.file.is_file())
        .map(Item::from)
        .collect()
}

/// `0:07`, `12:30`, `1:02:03`.
pub fn duration_text(secs: f64) -> String {
    let s = secs.max(0.).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

pub fn count(items: &[Item], category: Category) -> usize {
    items.iter().filter(|i| category.admits(i.kind)).count()
}

/// Search matches the name, ignoring case; ties in name or kind fall back to newest first.
pub fn visible(items: &[Item], category: Category, query: &str, sort: SortOrder) -> Vec<Item> {
    let needle = query.trim().to_lowercase();
    let mut shown: Vec<Item> = items
        .iter()
        .filter(|i| category.admits(i.kind))
        .filter(|i| needle.is_empty() || i.name.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    let newest = |a: &Item, b: &Item| b.created_at.cmp(&a.created_at);
    match sort {
        SortOrder::Name => shown.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| newest(a, b))
        }),
        SortOrder::Kind => shown.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| newest(a, b))),
        SortOrder::NewestFirst => shown.sort_by(newest),
        SortOrder::OldestFirst => shown.sort_by_key(|i| i.created_at),
    }
    shown
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Left,
    Right,
    Up,
    Down,
    First,
    Last,
}

/// The index selected after a key press; with nothing selected any step picks the first item.
pub fn step(selected: Option<usize>, step: Step, columns: usize, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let columns = columns.max(1);
    let last = len - 1;
    let Some(at) = selected.map(|s| s.min(last)) else {
        return Some(if step == Step::Last { last } else { 0 });
    };
    Some(match step {
        Step::Left => at.saturating_sub(1),
        Step::Right => (at + 1).min(last),
        Step::Up => at.checked_sub(columns).unwrap_or(at),
        Step::Down if at + columns <= last => at + columns,
        Step::Down if at / columns < last / columns => last,
        Step::Down => at,
        Step::First => 0,
        Step::Last => last,
    })
}

pub const GRID_PADDING: f32 = 24.;
pub const GRID_SPACING_X: f32 = 16.;
pub const GRID_SPACING_Y: f32 = 24.;
pub const ART_PAD: f32 = 10.;
pub const TITLE_HEIGHT: f32 = 36.;
/// Room beside the art for a two-line name.
const CELL_SLACK: f32 = 48.;

pub const SIZE_MIN: u32 = 64;
pub const SIZE_MAX: u32 = 256;
pub const SIZE_STEP: u32 = 16;

/// A hand-edited size outside the range draws at the nearest end.
pub fn item_size(raw: u32) -> u32 {
    raw.clamp(SIZE_MIN, SIZE_MAX)
}

/// One key press: to the next step mark in that direction.
pub fn size_step(size: u32, bigger: bool) -> u32 {
    let size = item_size(size);
    let next = if bigger {
        (size / SIZE_STEP + 1) * SIZE_STEP
    } else {
        (size - 1) / SIZE_STEP * SIZE_STEP
    };
    item_size(next)
}

pub fn row_height(size: u32) -> f32 {
    size as f32 + 2. * ART_PAD + 4. + TITLE_HEIGHT + GRID_SPACING_Y
}

/// Twice the drawn size so a scaled display stays sharp, in three cached sizes so resizing rarely decodes again.
pub fn thumb_pixels(size: u32) -> u32 {
    match 2 * item_size(size) {
        n if n <= 128 => 128,
        n if n <= 256 => 256,
        _ => 512,
    }
}

/// Columns of at least the cell width for `size` that fit the content width, gaps included.
pub fn columns_for(content_width: f32, size: u32) -> usize {
    let cell = item_size(size) as f32 + CELL_SLACK;
    let inner = content_width - 2. * GRID_PADDING + GRID_SPACING_X;
    ((inner / (cell + GRID_SPACING_X)).floor() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_from_a_short_last_row_stays_and_down_into_it_lands_on_the_last_item() {
        assert_eq!(step(Some(1), Step::Down, 4, 6), Some(5));
        assert_eq!(step(Some(5), Step::Down, 4, 6), Some(5));
        assert_eq!(step(Some(2), Step::Down, 4, 10), Some(6));
        assert_eq!(step(Some(1), Step::Up, 4, 10), Some(1));
        assert_eq!(step(None, Step::Up, 4, 10), Some(0));
        assert_eq!(step(None, Step::Down, 4, 0), None);
    }

    #[test]
    fn a_column_needs_its_cell_width_and_a_gap() {
        assert_eq!(columns_for(48. + 112., 64), 1);
        assert_eq!(columns_for(48. + 112. * 2. + 16., 64), 2);
        assert_eq!(columns_for(48. + 112. * 2. + 15., 64), 1);
        assert_eq!(columns_for(48. + 304. * 2. + 16., 256), 2);
        assert_eq!(columns_for(10., 256), 1);
    }

    #[test]
    fn the_default_window_shows_six_columns_at_the_smallest_size_and_two_at_the_largest() {
        let content = 1080. - super::super::render::SIDEBAR_WIDTH;
        let at = |size| columns_for(content, size);
        assert_eq!(at(SIZE_MIN), 6);
        assert_eq!(at(96), 5);
        assert_eq!(at(192), 3);
        assert_eq!(at(SIZE_MAX), 2);
        assert_eq!(at(1000), 2, "an out-of-range size is clamped");
        assert!((SIZE_MIN..=SIZE_MAX).all(|s| at(s) >= at(s + 1)));
    }

    #[test]
    fn key_steps_land_on_step_marks_and_stop_at_the_ends() {
        assert_eq!(size_step(96, true), 112);
        assert_eq!(size_step(100, true), 112);
        assert_eq!(size_step(100, false), 96);
        assert_eq!(size_step(96, false), 80);
        assert_eq!(size_step(SIZE_MAX, true), SIZE_MAX);
        assert_eq!(size_step(SIZE_MIN, false), SIZE_MIN);
        assert_eq!(size_step(5, true), 80);
    }

    #[test]
    fn thumbnails_are_cached_at_twice_the_drawn_size_or_more() {
        for size in SIZE_MIN..=SIZE_MAX {
            assert!(thumb_pixels(size) >= 2 * size, "{size}");
        }
        assert_eq!(thumb_pixels(SIZE_MIN), 128);
        assert_eq!(thumb_pixels(96), 256);
        assert_eq!(thumb_pixels(SIZE_MAX), 512);
    }
}
