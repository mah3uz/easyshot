use super::skill_install::{FILES, shared_dir};
use es_store::skills::{self, Refresh, State};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FIX_EDITED: &str = "easyshot agent skill install --force";
const FIX_OLD: &str = "run `easyshot agent skill install` once";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Edited,
    OutOfDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillNote {
    pub dir: PathBuf,
    pub kind: NoteKind,
}

impl SkillNote {
    pub fn fix(&self) -> &'static str {
        match self.kind {
            NoteKind::Edited => FIX_EDITED,
            NoteKind::OutOfDate => FIX_OLD,
        }
    }

    pub fn to_json(&self) -> Value {
        let state = match self.kind {
            NoteKind::Edited => "edited",
            NoteKind::OutOfDate => "out-of-date",
        };
        json!({"dir": self.dir, "state": state, "fix": self.fix()})
    }

    pub fn line(&self) -> String {
        let what = match self.kind {
            NoteKind::Edited => "was edited, so easyshot does not update it",
            NoteKind::OutOfDate => "is out of date",
        };
        format!("skill\t{} {what}. Fix: {}", self.dir.display(), self.fix())
    }
}

pub fn with_notes(mut value: Value, notes: &[SkillNote]) -> Value {
    if let Some(obj) = value.as_object_mut()
        && !notes.is_empty()
    {
        obj.insert(
            "skill".into(),
            Value::Array(notes.iter().map(SkillNote::to_json).collect()),
        );
    }
    value
}

/// Recorded installs that need attention, plus an unrecorded default install from before installs were recorded.
pub fn notes(record: &Path, home: Option<&Path>) -> Vec<SkillNote> {
    let assessed = skills::assess(record, &FILES).unwrap_or_default();
    let mut notes: Vec<SkillNote> = assessed
        .iter()
        .filter_map(|(dir, state)| {
            let kind = match state {
                State::Edited => NoteKind::Edited,
                State::Outdated => NoteKind::OutOfDate,
                State::Current | State::Gone => return None,
            };
            Some(SkillNote {
                dir: dir.clone(),
                kind,
            })
        })
        .collect();
    if let Some(dir) = home.map(shared_dir)
        && !assessed.iter().any(|(d, _)| *d == dir)
        && dir.join("SKILL.md").exists()
        && !skills::is_current(&dir, &FILES)
    {
        notes.push(SkillNote {
            dir,
            kind: NoteKind::OutOfDate,
        });
    }
    notes
}

/// Brings unedited recorded copies up to the skill this build carries.
pub fn refresh_installs(record: &Path) {
    match skills::refresh(record, &FILES) {
        Ok(report) => {
            for (dir, outcome) in report {
                match outcome {
                    Refresh::Replaced => {
                        tracing::info!(dir = %dir.display(), "agent skill refreshed");
                    }
                    Refresh::Edited => {
                        tracing::info!(dir = %dir.display(), "agent skill was edited; left as it is");
                    }
                    Refresh::Current | Refresh::Gone => {}
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "agent skill installs not refreshed"),
    }
}

#[cfg(test)]
#[path = "skill_refresh_tests.rs"]
mod tests;
