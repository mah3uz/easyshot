use crate::time_serde;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareKind {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareTarget {
    Free,
    Bucket { label: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareSource {
    Capture(Uuid),
    Recording(PathBuf),
}

/// What it takes to delete a share: the free service's token, or the stored bucket keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareDelete {
    FreeToken(String),
    Bucket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShareState {
    Uploading,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareRecord {
    pub id: String,
    pub target: ShareTarget,
    pub kind: ShareKind,
    pub page_url: String,
    pub media_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// None for a bucket share, which the user's own bucket rules expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<u8>,
    #[serde(with = "time_serde")]
    pub created_at: OffsetDateTime,
    #[serde(default, with = "time_serde::option")]
    pub expires_at: Option<OffsetDateTime>,
    pub source: ShareSource,
    pub delete: ShareDelete,
    pub state: ShareState,
}

impl ShareRecord {
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }
}

/// The `easyshot.shares` file: every link easyshot made, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shares {
    pub entries: Vec<ShareRecord>,
}

impl Shares {
    pub fn for_source(&self, source: &ShareSource, now: OffsetDateTime) -> Option<&ShareRecord> {
        self.entries
            .iter()
            .filter(|r| &r.source == source && !r.is_expired(now))
            .max_by_key(|r| r.created_at)
    }

    pub fn upsert(&mut self, record: ShareRecord) {
        match self.entries.iter_mut().find(|r| r.id == record.id) {
            Some(slot) => *slot = record,
            None => self.entries.insert(0, record),
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<ShareRecord> {
        let at = self.entries.iter().position(|r| r.id == id)?;
        Some(self.entries.remove(at))
    }

    pub fn expired(&self, now: OffsetDateTime) -> impl Iterator<Item = &ShareRecord> {
        self.entries.iter().filter(move |r| r.is_expired(now))
    }
}

/// The user's own bucket credentials, the file fallback for when no keyring is available.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BucketKeys {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl fmt::Debug for BucketKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BucketKeys")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"<hidden>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(secs).unwrap()
    }

    fn record(id: &str, source: &ShareSource, created: i64, expires: Option<i64>) -> ShareRecord {
        ShareRecord {
            id: id.into(),
            target: ShareTarget::Free,
            kind: ShareKind::Image,
            page_url: format!("https://x.example/{id}"),
            media_url: format!("https://m.example/{id}.png"),
            poster_url: None,
            title: None,
            days: Some(7),
            created_at: at(created),
            expires_at: expires.map(at),
            source: source.clone(),
            delete: ShareDelete::FreeToken("t".into()),
            state: ShareState::Ready,
        }
    }

    #[test]
    fn an_expired_share_is_not_offered_again_for_its_source() {
        let src = ShareSource::Capture(Uuid::nil());
        let mut shares = Shares::default();
        shares.upsert(record("old", &src, 100, Some(200)));
        assert!(shares.for_source(&src, at(150)).is_some());
        assert!(
            shares.for_source(&src, at(200)).is_none(),
            "a dead link must not be copied as if it worked"
        );
    }

    #[test]
    fn for_source_picks_the_newest_live_share() {
        let src = ShareSource::Recording("/v/a.easyshotrec".into());
        let other = ShareSource::Capture(Uuid::nil());
        let mut shares = Shares::default();
        shares.upsert(record("a", &src, 100, None));
        shares.upsert(record("b", &src, 300, None));
        shares.upsert(record("c", &src, 200, None));
        shares.upsert(record("d", &other, 400, None));
        assert_eq!(
            shares.for_source(&src, at(0)).map(|r| r.id.as_str()),
            Some("b")
        );
    }

    #[test]
    fn upsert_replaces_by_id_and_new_ones_go_first() {
        let src = ShareSource::Capture(Uuid::nil());
        let mut shares = Shares::default();
        shares.upsert(record("a", &src, 1, None));
        shares.upsert(record("b", &src, 2, None));
        let mut again = record("a", &src, 1, None);
        again.state = ShareState::Uploading;
        shares.upsert(again);
        let ids: Vec<_> = shares.entries.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(shares.entries[1].state, ShareState::Uploading);
    }

    #[test]
    fn remove_returns_the_record_and_expired_lists_only_dead_ones() {
        let src = ShareSource::Capture(Uuid::nil());
        let mut shares = Shares::default();
        shares.upsert(record("live", &src, 1, Some(500)));
        shares.upsert(record("dead", &src, 2, Some(50)));
        shares.upsert(record("forever", &src, 3, None));
        let dead: Vec<_> = shares.expired(at(100)).map(|r| r.id.as_str()).collect();
        assert_eq!(dead, ["dead"]);
        assert_eq!(shares.remove("dead").map(|r| r.id), Some("dead".into()));
        assert!(shares.remove("dead").is_none());
        assert_eq!(shares.entries.len(), 2);
    }

    #[test]
    fn debug_never_prints_the_secret() {
        let keys = BucketKeys {
            access_key_id: "AKIA123".into(),
            secret_access_key: "hunter2-secret".into(),
        };
        let shown = format!("{keys:?}");
        assert!(shown.contains("AKIA123"));
        assert!(!shown.contains("hunter2-secret"));
    }
}
