use crate::atomic::{ensure_private_dir, write_atomic, write_json_atomic};
use crate::error::{Error, Result, io};
use crate::paths::Paths;
use crate::thumbs;
use es_model::{CaptureKind, DeckManifest, Envelope, FormatKind, Header, Migrated, migrate};
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const MAX_AGE: Duration = Duration::hours(24);
const MANIFEST: &str = "item.json";

pub struct StageRequest<'a> {
    pub file_name: &'a str,
    pub kind: CaptureKind,
    pub history_id: Option<Uuid>,
    pub image: &'a [u8],
    pub raw: &'a [u8],
    pub thumb_size: u32,
}

pub struct Deck {
    paths: Paths,
}

fn read_manifest(dir: &Path) -> Option<DeckManifest> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let header: Header = serde_json::from_value(value.clone()).ok()?;
    if header.format != FormatKind::DeckItem.name() {
        return None;
    }
    match migrate(FormatKind::DeckItem, header.version, value).ok()? {
        Migrated::Current(v) => serde_json::from_value::<Envelope<DeckManifest>>(v)
            .ok()
            .map(|e| e.body),
        Migrated::NewerReadOnly { .. } => None,
    }
}

fn modified_before(dir: &Path, cutoff: OffsetDateTime) -> bool {
    std::fs::metadata(dir)
        .and_then(|m| m.modified())
        .is_ok_and(|m| OffsetDateTime::from(m) < cutoff)
}

impl Deck {
    pub fn new(paths: &Paths) -> Self {
        Self {
            paths: paths.clone(),
        }
    }

    fn item_dir(&self, id: Uuid) -> PathBuf {
        self.paths.deck_dir().join(id.to_string())
    }

    /// The manifest is written last, so a half-staged item has none and is purged once old.
    pub fn stage(&self, req: &StageRequest<'_>, now: OffsetDateTime) -> Result<DeckManifest> {
        let id = Uuid::new_v4();
        let dir = self.item_dir(id);
        ensure_private_dir(&dir)?;
        let name = Path::new(req.file_name)
            .file_name()
            .map_or_else(|| "capture.png".into(), |n| n.to_os_string());
        let staged = dir.join(&name);
        let stem = Path::new(&name)
            .file_stem()
            .map_or_else(|| "capture".into(), |s| s.to_string_lossy().into_owned());
        let raw = dir.join(format!("{stem}.raw.png"));
        write_atomic(&staged, req.image)?;
        write_atomic(&raw, req.raw)?;
        let modified = std::fs::metadata(&staged)
            .and_then(|m| m.modified())
            .map_err(io(&staged))?;
        let manifest = DeckManifest {
            id,
            name: stem,
            thumb: thumbs::thumb_path(&self.paths, &staged, modified, req.thumb_size),
            staged,
            raw,
            kind: req.kind,
            history_id: req.history_id,
            created_at: now,
        };
        write_json_atomic(
            &dir.join(MANIFEST),
            &Envelope::new(FormatKind::DeckItem, &manifest),
        )?;
        Ok(manifest)
    }

    /// Purges items a day old or more, returns the rest oldest first.
    pub fn restore(&self, now: OffsetDateTime) -> Result<Vec<DeckManifest>> {
        let root = self.paths.deck_dir();
        let read = match std::fs::read_dir(&root) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(Error::Io { path: root, source }),
        };
        let mut items = Vec::new();
        for entry in read.filter_map(|e| e.ok()) {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            match read_manifest(&dir) {
                Some(mut m) if now - m.created_at < MAX_AGE => {
                    let rebase = |p: &Path| dir.join(p.file_name().unwrap_or_default());
                    m.staged = rebase(&m.staged);
                    m.raw = rebase(&m.raw);
                    if m.staged.exists() {
                        items.push(m);
                    } else {
                        let _ = std::fs::remove_dir_all(&dir);
                    }
                }
                Some(_) => {
                    let _ = std::fs::remove_dir_all(&dir);
                }
                None if modified_before(&dir, now - MAX_AGE) => {
                    let _ = std::fs::remove_dir_all(&dir);
                }
                None => {}
            }
        }
        items.sort_by_key(|m| m.created_at);
        Ok(items)
    }

    /// Call after the item is saved or discarded.
    pub fn remove(&self, id: Uuid) -> Result<()> {
        let dir = self.item_dir(id);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(&dir)(e)),
        }
    }
}
