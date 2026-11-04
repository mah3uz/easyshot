use es_model::HistoryEntry;
use es_store::Paths;
use es_store::media::{self, MediaFile};
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

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
pub const CELL_MIN_WIDTH: f32 = 112.;

/// Columns of at least the minimum cell width that fit the content width, gaps included.
pub fn columns_for(content_width: f32) -> usize {
    let inner = content_width - 2. * GRID_PADDING + GRID_SPACING_X;
    ((inner / (CELL_MIN_WIDTH + GRID_SPACING_X)).floor() as usize).max(1)
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
    fn a_column_needs_its_minimum_width_and_a_gap() {
        assert_eq!(columns_for(48. + 112.), 1);
        assert_eq!(columns_for(48. + 112. * 2. + 16.), 2);
        assert_eq!(columns_for(48. + 112. * 2. + 15.), 1);
        assert_eq!(columns_for(10.), 1);
    }
}
