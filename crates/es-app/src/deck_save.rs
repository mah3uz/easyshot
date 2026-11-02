use crate::deck::DeckItem;
use crate::errors::{Error, Result};
use crate::pipeline::{Work, pixmap, retain, save_to_folder};
use es_model::{Config, HistoryEntry};
use es_store::Paths;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Saved {
    pub file: PathBuf,
    /// Present when the capture was only staged and now also lives in the library.
    pub entry: Option<HistoryEntry>,
}

/// Writes a deck card to the save folder; a staged card also joins the library and its staging is removed.
pub fn save_card(paths: &Paths, config: &Arc<Config>, item: &DeckItem) -> Result<Saved> {
    let png = std::fs::read(&item.file).map_err(|source| Error::Io {
        path: item.file.clone(),
        source,
    })?;
    let rgba = image::load_from_memory(&png)
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
    };
    let file = save_to_folder(&work, &pixmap(&rgba)?)?;
    let entry = if item.staged {
        let entry = retain(&work, &png, Some(&file))?;
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
