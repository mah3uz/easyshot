use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A folder easyshot wrote the agent skill into, and a hash of the files as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInstall {
    pub dir: PathBuf,
    pub hash: String,
    #[serde(default)]
    pub stale: bool,
}

/// The `easyshot.skill-installs` file: where the skill was installed, so an upgrade can refresh it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillInstalls {
    pub installs: Vec<SkillInstall>,
}
