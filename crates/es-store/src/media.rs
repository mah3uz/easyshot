use crate::paths::Paths;
use es_model::HistoryEntry;
use es_model::recording::{RecordingCapture, RecordingProject};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

const FOLDER_SUFFIX: &str = "easyshotrec";
const VIDEO_EXTENSIONS: [&str; 5] = ["mp4", "webm", "mkv", "mov", "gif"];
const PROJECT_VIDEOS: [&str; 2] = ["screen.mkv", "screen.mp4"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaKind {
    Image,
    Video,
}

/// One file in the library: a screenshot, a saved or exported video, or a recording not yet saved.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaFile {
    pub id: Uuid,
    pub kind: MediaKind,
    pub name: String,
    pub file: PathBuf,
    pub created_at: OffsetDateTime,
    pub duration_secs: Option<f64>,
    /// The recording project folder of a video that has not been saved; deleting the video deletes it.
    pub project: Option<PathBuf>,
}

pub fn is_video(file: &Path) -> bool {
    file.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn id_for(file: &Path) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, file.as_os_str().as_encoded_bytes())
}

fn modified(file: &Path) -> Option<OffsetDateTime> {
    let time = std::fs::metadata(file).and_then(|m| m.modified()).ok()?;
    Some(OffsetDateTime::from(time))
}

fn read_json<T: serde::de::DeserializeOwned>(file: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

fn saved_video(file: PathBuf) -> Option<MediaFile> {
    let created_at = modified(&file)?;
    let name = file.file_name()?.to_string_lossy().into_owned();
    Some(MediaFile {
        id: id_for(&file),
        kind: MediaKind::Video,
        name,
        file,
        created_at,
        duration_secs: None,
        project: None,
    })
}

/// A finished project has a `capture.json`; one without it is still being recorded or waits for recovery.
fn project_video(dir: PathBuf) -> Option<MediaFile> {
    let capture: RecordingCapture = read_json(&dir.join("capture.json"))?;
    let file = std::iter::once(capture.video_file.as_str())
        .chain(PROJECT_VIDEOS)
        .map(|name| dir.join(name))
        .find(|f| f.is_file())?;
    let name = read_json::<RecordingProject>(&dir.join("project.json"))
        .map(|p| p.display_name)
        .filter(|n| !n.is_empty())
        .or_else(|| dir.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    Some(MediaFile {
        id: id_for(&file),
        kind: MediaKind::Video,
        name,
        created_at: modified(&file).unwrap_or(capture.created_at),
        duration_secs: Some(capture.duration_secs).filter(|d| *d > 0.),
        project: Some(dir),
        file,
    })
}

fn entries(dir: &Path) -> impl Iterator<Item = PathBuf> + use<> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
}

/// Saved videos and exports from the videos folder plus unsaved recording projects, newest first.
pub fn videos(paths: &Paths) -> Vec<MediaFile> {
    let saved = entries(&paths.videos_dir())
        .filter(|p| is_video(p))
        .filter_map(saved_video);
    let unsaved = entries(&paths.recordings_dir())
        .filter(|p| p.is_dir() && p.extension().is_some_and(|e| e == FOLDER_SUFFIX))
        .filter_map(project_video);
    let mut all: Vec<MediaFile> = saved.chain(unsaved).collect();
    all.sort_by_key(|m| std::cmp::Reverse(m.created_at));
    all
}

/// The images given, as the history keeps them, and every video, newest first.
pub fn list(paths: &Paths, images: &[HistoryEntry]) -> Vec<MediaFile> {
    let mut all: Vec<MediaFile> = images
        .iter()
        .filter(|e| !is_video(&e.file))
        .map(|e| MediaFile {
            id: e.id,
            kind: MediaKind::Image,
            name: e.name.clone(),
            file: e.file.clone(),
            created_at: e.created_at,
            duration_secs: None,
            project: None,
        })
        .collect();
    all.extend(videos(paths));
    all.sort_by_key(|m| std::cmp::Reverse(m.created_at));
    all
}
