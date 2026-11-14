use crate::time_serde;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const MAX_LOG_LIMIT: u32 = 10_000;

/// When the daemon asks the user before answering an agent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consent {
    #[default]
    OncePerSession,
    AlwaysAsk,
    AlwaysAllow,
}

impl Consent {
    pub const ALL: [Consent; 3] = [
        Consent::OncePerSession,
        Consent::AlwaysAsk,
        Consent::AlwaysAllow,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Consent::OncePerSession => "once-per-session",
            Consent::AlwaysAsk => "always-ask",
            Consent::AlwaysAllow => "always-allow",
        }
    }

    pub fn from_name(name: &str) -> Option<Consent> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentCfg {
    pub enabled: bool,
    pub consent: Consent,
    /// Output names an agent may capture; empty means every output.
    pub allowed_outputs: Vec<String>,
    /// App ids an agent may capture; empty means every window.
    pub allowed_windows: Vec<String>,
    pub redact_secrets: bool,
    pub log_limit: u32,
    pub keep_captures: bool,
}

impl Default for AgentCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            consent: Consent::OncePerSession,
            allowed_outputs: Vec::new(),
            allowed_windows: Vec::new(),
            redact_secrets: true,
            log_limit: 200,
            keep_captures: true,
        }
    }
}

impl AgentCfg {
    /// A hand-edited limit outside the range still keeps the log bounded.
    pub fn log_limit(&self) -> usize {
        self.log_limit.clamp(1, MAX_LOG_LIMIT) as usize
    }

    /// `temp` can only drop a capture; it never keeps one the user's setting drops.
    pub fn keeps(&self, temp: bool) -> bool {
        self.keep_captures && !temp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlwaysGrant {
    #[serde(with = "time_serde")]
    pub granted_at: OffsetDateTime,
}

/// One agent request and what became of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    #[serde(with = "time_serde")]
    pub at: OffsetDateTime,
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `ok`, or the error code the agent was answered with.
    pub result: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comm: Option<String>,
}

/// The `easyshot.agent` file: the lasting grant and the recent actions, written by the daemon only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentRecord {
    pub always: Option<AlwaysGrant>,
    /// Newest first.
    pub log: Vec<ActionRecord>,
}

impl AgentRecord {
    pub fn push(&mut self, entry: ActionRecord, limit: usize) {
        self.log.insert(0, entry);
        self.log.truncate(limit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: u32) -> ActionRecord {
        ActionRecord {
            at: OffsetDateTime::UNIX_EPOCH,
            route: format!("agent/{n}"),
            target: None,
            result: "ok".into(),
            pid: Some(n),
            comm: None,
        }
    }

    #[test]
    fn access_is_off_and_asks_once_unless_the_user_says_otherwise() {
        let c: AgentCfg = toml::from_str("").unwrap();
        assert_eq!(c, AgentCfg::default());
        assert!(!c.enabled, "agent access is opt-in");
        assert_eq!(c.consent, Consent::OncePerSession);
        assert!(c.allowed_outputs.is_empty() && c.allowed_windows.is_empty());
        assert!(c.redact_secrets);
        assert!(
            c.keep_captures,
            "agent captures reach the gallery unless the user opts out"
        );
        assert!(
            !toml::from_str::<AgentCfg>("keep_captures = false")
                .unwrap()
                .keep_captures
        );
    }

    #[test]
    fn a_temp_capture_is_dropped_but_never_kept_against_the_users_setting() {
        let on = AgentCfg::default();
        let off = AgentCfg {
            keep_captures: false,
            ..AgentCfg::default()
        };
        assert!(on.keeps(false));
        assert!(!on.keeps(true));
        assert!(!off.keeps(false));
        assert!(!off.keeps(true));
    }

    #[test]
    fn consent_names_are_the_ones_written_in_the_file() {
        for c in Consent::ALL {
            let toml = format!("consent = \"{}\"", c.name());
            assert_eq!(toml::from_str::<AgentCfg>(&toml).unwrap().consent, c);
            assert_eq!(Consent::from_name(c.name()), Some(c));
        }
        assert!(toml::from_str::<AgentCfg>("consent = \"never\"").is_err());
    }

    #[test]
    fn the_log_is_newest_first_and_bounded() {
        let mut r = AgentRecord::default();
        for n in 0..5 {
            r.push(entry(n), 3);
        }
        let pids: Vec<_> = r.log.iter().map(|e| e.pid).collect();
        assert_eq!(pids, [Some(4), Some(3), Some(2)]);
    }

    #[test]
    fn a_hand_edited_log_limit_is_kept_in_range() {
        let mut c = AgentCfg {
            log_limit: 0,
            ..AgentCfg::default()
        };
        assert_eq!(c.log_limit(), 1);
        c.log_limit = u32::MAX;
        assert_eq!(c.log_limit(), MAX_LOG_LIMIT as usize);
    }
}
