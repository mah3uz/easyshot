use crate::deck::DeckItem;
use crate::errors::{Error, Result};
use crate::pipeline::{Work, new_capture_look, pixmap, retain, save_to_folder};
use es_model::{Config, HistoryEntry};
use es_store::Paths;
use es_store::temp::{self, TempKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Saved {
    pub file: PathBuf,
    /// Present when the capture was only staged and now also lives in the library.
    pub entry: Option<HistoryEntry>,
}

/// The untouched pixels staged beside a card's image.
fn raw_path(staged: &Path) -> PathBuf {
    let stem = staged
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    staged.with_file_name(format!("{stem}.raw.png"))
}

fn detached(paths: &Paths, item: &DeckItem, from: &Path) -> Result<PathBuf> {
    let bytes = std::fs::read(from).map_err(|source| Error::Io {
        path: from.to_owned(),
        source,
    })?;
    let name = item.file.file_name().map_or_else(
        || "capture.png".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    Ok(temp::create(paths, TempKind::Unstaged, &name, &bytes)?)
}

/// The file the editor opens for a card. A staged card shows the capture inside its look, so the
/// editor gets a copy of the raw pixels and applies the look itself instead of stacking a second.
/// The copy lives outside the deck's staging, which goes with the card.
pub fn editable(paths: &Paths, item: &DeckItem) -> Result<PathBuf> {
    if !item.staged {
        return Ok(item.file.clone());
    }
    let raw = raw_path(&item.file);
    detached(paths, item, if raw.exists() { &raw } else { &item.file })
}

/// The file a pin holds, outside the deck's staging so an unsaved pin never returns as a card.
pub fn pinnable(paths: &Paths, item: &DeckItem) -> Result<PathBuf> {
    if item.staged {
        detached(paths, item, &item.file)
    } else {
        Ok(item.file.clone())
    }
}

/// Writes a deck card to the save folder; a staged card also joins the library and its staging is removed.
pub fn save_card(paths: &Paths, config: &Arc<Config>, item: &DeckItem) -> Result<Saved> {
    let png = std::fs::read(&item.file).map_err(|source| Error::Io {
        path: item.file.clone(),
        source,
    })?;
    let rgba = es_store::decode::image_from_memory(&png)
        .map_err(|e| Error::Protocol(format!("{}: {e}", item.file.display())))?
        .to_rgba8();
    let stem = Path::new(&item.name)
        .file_stem()
        .or_else(|| item.file.file_stem())
        .map_or_else(
            || "capture".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
    let work = Work {
        paths: paths.clone(),
        config: config.clone(),
        file_stem: stem,
        kind: item.kind,
        save: true,
        show: false,
    };
    let file = save_to_folder(&work, &pixmap(&rgba)?)?;
    let entry = if item.staged {
        let raw = std::fs::read(raw_path(&item.file))
            .ok()
            .filter(|r| *r != png);
        let look = raw.as_deref().map(|r| (r, new_capture_look(paths, config)));
        let entry = retain(&work, &png, look, Some(&file))?;
        if let Err(e) = es_store::Deck::new(paths).remove(item.id) {
            tracing::warn!(error = %e, "deck staging not removed");
        }
        Some(entry)
    } else {
        None
    };
    Ok(Saved { file, entry })
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_model::CaptureKind;
    use es_store::StageRequest;
    use time::OffsetDateTime;

    fn png() -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            8,
            6,
            image::Rgba([9, 80, 200, 255]),
        ))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
        out
    }

    fn staged(paths: &Paths) -> DeckItem {
        let png = png();
        let manifest = es_store::Deck::new(paths)
            .stage(
                &StageRequest {
                    file_name: "Shot.png",
                    kind: CaptureKind::Region,
                    history_id: None,
                    image: &png,
                    raw: &png,
                    thumb_size: 64,
                },
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        DeckItem::staged(&manifest)
    }

    #[test]
    fn a_staged_card_lands_in_the_save_folder_and_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let mut config = Config::default();
        config.general.save_dir = Some(dir.path().join("saved"));
        let item = staged(&paths);

        let saved = save_card(&paths, &Arc::new(config), &item).unwrap();

        assert!(saved.file.starts_with(dir.path().join("saved")));
        assert_eq!(image::image_dimensions(&saved.file).unwrap(), (8, 6));
        let entry = saved.entry.expect("staged cards join the library");
        assert!(entry.file.starts_with(paths.library_dir()));
        assert_eq!(entry.export_path.as_deref(), Some(saved.file.as_path()));
        assert!(
            !item.file.exists(),
            "staging is removed once the card is kept"
        );
    }

    fn composed_card(paths: &Paths) -> DeckItem {
        let (raw, shown) = (png(), {
            let mut out = Vec::new();
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                12,
                10,
                image::Rgba([200, 0, 0, 255]),
            ))
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
            out
        });
        let manifest = es_store::Deck::new(paths)
            .stage(
                &StageRequest {
                    file_name: "Shot.png",
                    kind: CaptureKind::Region,
                    history_id: None,
                    image: &shown,
                    raw: &raw,
                    thumb_size: 64,
                },
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        DeckItem::staged(&manifest)
    }

    #[test]
    fn a_composed_card_joins_the_library_with_its_raw_base_and_look() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let mut presets = es_store::presets::open_backgrounds(&paths).unwrap();
        let mut preset = es_model::BackgroundPreset::default();
        preset.background.style = es_model::BackgroundStyle::Solid(es_model::BackgroundColor {
            id: "r".into(),
            title: "R".into(),
            red: 1.,
            green: 0.,
            blue: 0.,
            alpha: 1.,
        });
        presets.value.active_preset_id = Some(preset.id);
        presets.value.presets.push(preset);
        presets.save().unwrap();
        let mut config = Config::default();
        config.general.save_dir = Some(dir.path().join("saved"));
        let item = composed_card(&paths);

        let saved = save_card(&paths, &Arc::new(config), &item).unwrap();

        let entry = saved.entry.unwrap();
        assert_eq!(image::image_dimensions(&entry.file).unwrap(), (12, 10));
        let reopened = es_store::reopen(&entry.file);
        assert!(matches!(
            reopened.document.unwrap().background.style,
            es_model::BackgroundStyle::Solid(_)
        ));
        assert_eq!(image::image_dimensions(&reopened.source).unwrap(), (8, 6));
        assert_eq!(es_store::library::scan(&paths.library_dir()).len(), 1);
    }

    #[test]
    fn editing_or_pinning_a_staged_card_works_on_a_copy_outside_the_staging() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let item = composed_card(&paths);

        let edit = editable(&paths, &item).unwrap();
        let pin = pinnable(&paths, &item).unwrap();

        assert_eq!(
            image::image_dimensions(&edit).unwrap(),
            (8, 6),
            "the raw pixels"
        );
        assert_eq!(
            image::image_dimensions(&pin).unwrap(),
            (12, 10),
            "what the card shows"
        );
        for copy in [&edit, &pin] {
            assert!(copy.starts_with(paths.unstaged_dir()));
            assert_eq!(copy.file_name().unwrap(), "Shot.png");
        }
        let mut kept = item.clone();
        kept.staged = false;
        assert_eq!(editable(&paths, &kept).unwrap(), item.file);
        assert_eq!(pinnable(&paths, &kept).unwrap(), item.file);
    }

    #[test]
    fn a_card_already_in_the_library_is_only_exported() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let mut config = Config::default();
        config.general.save_dir = Some(dir.path().join("saved"));
        let mut item = staged(&paths);
        item.staged = false;

        let saved = save_card(&paths, &Arc::new(config), &item).unwrap();

        assert!(saved.entry.is_none());
        assert!(item.file.exists());
    }
}
