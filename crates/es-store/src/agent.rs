use crate::json::JsonFile;
use crate::paths::Paths;
use es_model::{AgentRecord, FormatKind};

pub type AgentFile = JsonFile<AgentRecord>;

/// A damaged file starts over empty, so a grant it may have held is dropped rather than trusted.
pub fn open(paths: &Paths) -> AgentFile {
    let path = paths.agent_file();
    JsonFile::open(&path, FormatKind::Agent).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "agent record unreadable; starting a new one");
        JsonFile::fresh(&path, FormatKind::Agent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_model::{ActionRecord, AlwaysGrant};
    use time::OffsetDateTime;

    #[test]
    fn a_grant_and_the_log_survive_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let mut file = open(&paths);
        file.value.always = Some(AlwaysGrant {
            granted_at: OffsetDateTime::UNIX_EPOCH,
        });
        file.value.push(
            ActionRecord {
                at: OffsetDateTime::UNIX_EPOCH,
                route: "agent/list-outputs".into(),
                target: None,
                result: "ok".into(),
                pid: Some(7),
                comm: Some("claude".into()),
            },
            10,
        );
        file.save().unwrap();
        assert_eq!(open(&paths).value, file.value);
    }

    #[test]
    fn a_damaged_file_grants_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        std::fs::write(paths.agent_file(), "{\"format\":\"easyshot.agent\",").unwrap();
        assert_eq!(open(&paths).value, AgentRecord::default());
    }
}
