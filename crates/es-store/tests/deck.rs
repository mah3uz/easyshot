#![allow(clippy::unwrap_used)]
use es_model::CaptureKind;
use es_store::{Deck, Paths, StageRequest};
use time::{Duration, OffsetDateTime};

fn setup() -> (tempfile::TempDir, Paths, Deck) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let deck = Deck::new(&paths);
    (dir, paths, deck)
}

fn stage(deck: &Deck, name: &str, at: OffsetDateTime) -> es_model::DeckManifest {
    deck.stage(
        &StageRequest {
            file_name: name,
            kind: CaptureKind::Region,
            history_id: None,
            image: b"image",
            raw: b"raw",
            thumb_size: 240,
        },
        at,
    )
    .unwrap()
}

#[test]
fn staging_writes_the_item_files_and_manifest() {
    let (_d, paths, deck) = setup();
    let now = OffsetDateTime::now_utc();
    let item = stage(&deck, "shot.jpg", now);
    let dir = paths.deck_dir().join(item.id.to_string());
    assert_eq!(item.staged, dir.join("shot.jpg"));
    assert_eq!(item.raw, dir.join("shot.raw.png"));
    assert_eq!(std::fs::read(&item.staged).unwrap(), b"image");
    assert_eq!(std::fs::read(&item.raw).unwrap(), b"raw");
    assert!(dir.join("item.json").exists());
    assert!(item.thumb.starts_with(paths.thumbs_dir()));
}

#[test]
fn items_younger_than_a_day_are_restored_and_older_are_purged() {
    let (_d, paths, deck) = setup();
    let now = OffsetDateTime::now_utc();
    let fresh = stage(&deck, "fresh.png", now - Duration::hours(23));
    let stale = stage(&deck, "stale.png", now - Duration::hours(25));
    let edge = stage(&deck, "edge.png", now - Duration::hours(24));

    let restored = deck.restore(now).unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, fresh.id);
    assert!(fresh.staged.exists());
    for gone in [&stale, &edge] {
        assert!(!paths.deck_dir().join(gone.id.to_string()).exists());
    }
}

#[test]
fn restore_lists_oldest_first() {
    let (_d, _p, deck) = setup();
    let now = OffsetDateTime::now_utc();
    let b = stage(&deck, "b.png", now - Duration::hours(1));
    let a = stage(&deck, "a.png", now - Duration::hours(3));
    let ids: Vec<_> = deck.restore(now).unwrap().iter().map(|m| m.id).collect();
    assert_eq!(ids, [a.id, b.id]);
}

#[test]
fn removing_an_item_takes_its_image_and_raw_with_it() {
    let (_d, paths, deck) = setup();
    let item = stage(&deck, "shot.png", OffsetDateTime::now_utc());
    deck.remove(item.id).unwrap();
    assert!(!paths.deck_dir().join(item.id.to_string()).exists());
    deck.remove(item.id).unwrap();
}

#[test]
fn a_half_staged_item_survives_until_it_is_old() {
    let (_d, paths, deck) = setup();
    let half = paths
        .deck_dir()
        .join("11111111-1111-1111-1111-111111111111");
    std::fs::create_dir_all(&half).unwrap();
    std::fs::write(half.join("x.png"), "x").unwrap();
    let now = OffsetDateTime::now_utc();
    assert!(deck.restore(now).unwrap().is_empty());
    assert!(half.exists(), "a staging in progress is left alone");
    assert!(deck.restore(now + Duration::hours(30)).unwrap().is_empty());
    assert!(!half.exists());
}

#[test]
fn staged_paths_follow_the_deck_when_the_data_dir_moves() {
    let (dir, paths, deck) = setup();
    let now = OffsetDateTime::now_utc();
    let item = stage(&deck, "shot.png", now);
    let moved = dir.path().join("moved");
    std::fs::rename(paths.deck_dir(), &moved).unwrap();
    std::fs::create_dir_all(paths.deck_dir()).unwrap();
    for e in std::fs::read_dir(&moved).unwrap() {
        let e = e.unwrap();
        std::fs::rename(e.path(), paths.deck_dir().join(e.file_name())).unwrap();
    }
    let restored = deck.restore(now).unwrap();
    assert_eq!(restored[0].staged, item.staged);
}
