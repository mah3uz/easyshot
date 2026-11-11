#![allow(clippy::unwrap_used)]
use es_model::{
    BucketKeys, ShareDelete, ShareKind, ShareRecord, ShareSource, ShareState, ShareTarget,
};
use es_store::{Error, Paths, shares};
use std::os::unix::fs::PermissionsExt;
use time::OffsetDateTime;

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn record() -> ShareRecord {
    ShareRecord {
        id: "abc".into(),
        target: ShareTarget::Free,
        kind: ShareKind::Image,
        page_url: "https://x.example/abc".into(),
        media_url: "https://m.example/abc.png".into(),
        poster_url: None,
        title: Some("t".into()),
        days: Some(7),
        created_at: OffsetDateTime::UNIX_EPOCH,
        expires_at: Some(OffsetDateTime::UNIX_EPOCH + time::Duration::days(7)),
        source: ShareSource::Capture(uuid::Uuid::nil()),
        delete: ShareDelete::FreeToken("secret-token".into()),
        state: ShareState::Ready,
    }
}

#[test]
fn shares_round_trip_and_the_file_is_private() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let mut f = shares::open(&paths).unwrap();
    assert!(f.value.entries.is_empty());
    f.value.upsert(record());
    f.save().unwrap();
    assert_eq!(shares::open(&paths).unwrap().value, f.value);
    let text = std::fs::read_to_string(paths.shares_file()).unwrap();
    assert!(text.contains("\"format\": \"easyshot.shares\""));
    assert_eq!(
        mode(&paths.shares_file()),
        0o600,
        "the file holds delete tokens"
    );
}

#[test]
fn newer_shares_open_read_only_and_stay_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    std::fs::create_dir_all(paths.shares_file().parent().unwrap()).unwrap();
    let body = r#"{"format":"easyshot.shares","version":9,"entries":[{"future":1}]}"#;
    std::fs::write(paths.shares_file(), body).unwrap();
    let f = shares::open(&paths).unwrap();
    assert_eq!(f.newer_version(), Some(9));
    assert!(matches!(f.save(), Err(Error::NewerVersion { version: 9 })));
    assert_eq!(std::fs::read_to_string(paths.shares_file()).unwrap(), body);
}

#[test]
fn the_install_token_is_written_read_and_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    assert_eq!(shares::read_install_token(&paths).unwrap(), None);
    shares::write_install_token(&paths, "tok-1\n").unwrap();
    assert_eq!(
        shares::read_install_token(&paths).unwrap().as_deref(),
        Some("tok-1")
    );
    assert_eq!(mode(&paths.share_install_token_file()), 0o600);
    shares::write_install_token(&paths, "tok-2").unwrap();
    assert_eq!(
        shares::read_install_token(&paths).unwrap().as_deref(),
        Some("tok-2")
    );
    shares::clear_install_token(&paths).unwrap();
    assert_eq!(shares::read_install_token(&paths).unwrap(), None);
    shares::clear_install_token(&paths).unwrap();
}

#[test]
fn bucket_keys_are_written_private_read_back_and_cleared() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    assert_eq!(shares::read_bucket_keys(&paths).unwrap(), None);
    let keys = BucketKeys {
        access_key_id: "AKIA1".into(),
        secret_access_key: "s3cret".into(),
    };
    shares::write_bucket_keys(&paths, keys.clone()).unwrap();
    assert_eq!(shares::read_bucket_keys(&paths).unwrap(), Some(keys));
    assert_eq!(
        mode(&paths.bucket_keys_file()),
        0o600,
        "the secret key must not be readable by other users"
    );
    assert!(
        !paths.bucket_keys_file().starts_with(paths.config_dir()),
        "config folders are often synced as dotfiles, so the secret must live elsewhere"
    );
    let text = std::fs::read_to_string(paths.bucket_keys_file()).unwrap();
    assert!(text.contains("\"format\": \"easyshot.bucket-keys\""));
    shares::clear_bucket_keys(&paths).unwrap();
    assert_eq!(shares::read_bucket_keys(&paths).unwrap(), None);
    shares::clear_bucket_keys(&paths).unwrap();
}
