use super::{ReplyKind, VERSION_ARG};
use crate::{ArgValue, RouteArgs};
use serde::{Deserialize, Serialize};

const KEY_ARG: &str = "key";
const VALUE_ARG: &str = "value";
const MAX_VALUE: usize = 4096;

/// The `[agent]` settings an agent command may read or change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentKey {
    Enabled,
    Consent,
    AllowedOutputs,
    AllowedWindows,
    RedactSecrets,
    LogLimit,
    KeepCaptures,
}

impl AgentKey {
    pub const ALL: [AgentKey; 7] = [
        AgentKey::Enabled,
        AgentKey::Consent,
        AgentKey::AllowedOutputs,
        AgentKey::AllowedWindows,
        AgentKey::RedactSecrets,
        AgentKey::LogLimit,
        AgentKey::KeepCaptures,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AgentKey::Enabled => "enabled",
            AgentKey::Consent => "consent",
            AgentKey::AllowedOutputs => "allowed_outputs",
            AgentKey::AllowedWindows => "allowed_windows",
            AgentKey::RedactSecrets => "redact_secrets",
            AgentKey::LogLimit => "log_limit",
            AgentKey::KeepCaptures => "keep_captures",
        }
    }

    pub fn from_name(name: &str) -> Option<AgentKey> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// Routes that read or change agent access; they work while access is off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManageRoute {
    Enable,
    Disable,
    Status,
    ConfigGet(AgentKey),
    ConfigSet(AgentKey, String),
}

impl ManageRoute {
    const NAMES: [&'static str; 5] = [
        "agent/enable",
        "agent/disable",
        "agent/status",
        "agent/config-get",
        "agent/config-set",
    ];

    pub(super) fn is_name(name: &str) -> bool {
        Self::NAMES.contains(&name)
    }

    pub fn route_name(&self) -> &'static str {
        let i = match self {
            ManageRoute::Enable => 0,
            ManageRoute::Disable => 1,
            ManageRoute::Status => 2,
            ManageRoute::ConfigGet(_) => 3,
            ManageRoute::ConfigSet(..) => 4,
        };
        Self::NAMES[i]
    }

    pub fn kind(&self) -> ReplyKind {
        match self {
            ManageRoute::Enable | ManageRoute::Disable | ManageRoute::Status => ReplyKind::Access,
            ManageRoute::ConfigGet(_) | ManageRoute::ConfigSet(..) => ReplyKind::Config,
        }
    }

    /// The value of `config set` is left out: it is not a fixed word.
    pub fn command_args(&self) -> Vec<&'static str> {
        match self {
            ManageRoute::Enable => vec!["agent", "enable"],
            ManageRoute::Disable => vec!["agent", "disable"],
            ManageRoute::Status => vec!["agent", "status"],
            ManageRoute::ConfigGet(k) => vec!["agent", "config", "get", k.name()],
            ManageRoute::ConfigSet(k, _) => vec!["agent", "config", "set", k.name()],
        }
    }

    pub(super) fn write_args(&self, args: &mut RouteArgs) {
        let mut put = |k: &str, v: &str| {
            args.0.insert(k.to_owned(), ArgValue::Str(v.to_owned()));
        };
        match self {
            ManageRoute::ConfigGet(key) => put(KEY_ARG, key.name()),
            ManageRoute::ConfigSet(key, value) => {
                put(KEY_ARG, key.name());
                put(VALUE_ARG, value);
            }
            ManageRoute::Enable | ManageRoute::Disable | ManageRoute::Status => {}
        }
    }

    /// The protocol version has already been checked.
    pub(super) fn parse(name: &str, args: &RouteArgs) -> Result<ManageRoute, String> {
        let string = |k: &str| match args.0.get(k) {
            Some(ArgValue::Str(s)) => Ok(s.as_str()),
            _ => Err(format!("{k} must be a string")),
        };
        let key = || {
            let name = string(KEY_ARG)?;
            AgentKey::from_name(name).ok_or_else(|| format!("{name:?} is not an agent setting"))
        };
        let (route, takes): (ManageRoute, &[&str]) = match name {
            "agent/enable" => (ManageRoute::Enable, &[]),
            "agent/disable" => (ManageRoute::Disable, &[]),
            "agent/status" => (ManageRoute::Status, &[]),
            "agent/config-get" => (ManageRoute::ConfigGet(key()?), &[KEY_ARG]),
            "agent/config-set" => {
                let value = string(VALUE_ARG)?;
                if value.len() > MAX_VALUE || value.chars().any(char::is_control) {
                    return Err(format!(
                        "a value has at most {MAX_VALUE} bytes and no control characters"
                    ));
                }
                (
                    ManageRoute::ConfigSet(key()?, value.to_owned()),
                    &[KEY_ARG, VALUE_ARG],
                )
            }
            _ => return Err(format!("{name} is not an agent access route")),
        };
        match args
            .0
            .keys()
            .find(|k| *k != VERSION_ARG && !takes.contains(&k.as_str()))
        {
            Some(extra) => Err(format!("unknown argument {extra}")),
            None => Ok(route),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantSpan {
    Hour,
    /// Until the daemon stops, which is at logout.
    Session,
    Always,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantStatus {
    pub span: GrantSpan,
    /// RFC 3339; only an hour's grant ends on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessStatus {
    pub enabled: bool,
    pub consent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<GrantStatus>,
    /// Empty means every output.
    pub allowed_outputs: Vec<String>,
    /// Empty means every window.
    pub allowed_windows: Vec<String>,
    pub redact_secrets: bool,
    #[serde(default = "default_keep")]
    pub keep_captures: bool,
    pub recent_actions: usize,
}

fn default_keep() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRoute;
    use crate::{Error, Route};

    fn examples() -> Vec<ManageRoute> {
        let mut all = vec![
            ManageRoute::Enable,
            ManageRoute::Disable,
            ManageRoute::Status,
        ];
        for key in AgentKey::ALL {
            all.push(ManageRoute::ConfigGet(key));
            all.push(ManageRoute::ConfigSet(key, "DP-1, HDMI-A-1".into()));
        }
        all
    }

    #[test]
    fn every_access_route_round_trips_through_its_name_and_args() {
        for manage in examples() {
            let route = Route::Agent(AgentRoute::Manage(manage.clone()));
            assert_eq!(route.as_str(), manage.route_name());
            assert_eq!(Route::parse(&route.as_str(), &route.args()).unwrap(), route);
        }
    }

    #[test]
    fn an_unknown_setting_extra_argument_or_control_character_is_refused() {
        let with = |name: &str, pairs: &[(&str, &str)]| {
            let mut args = AgentRoute::Manage(ManageRoute::Status).args();
            for (k, v) in pairs {
                args.0.insert((*k).into(), ArgValue::Str((*v).into()));
            }
            Route::parse(name, &args)
        };
        let invalid = |r: crate::Result<Route>| matches!(r, Err(Error::InvalidArgs { .. }));
        assert!(invalid(with("agent/config-get", &[("key", "network")])));
        assert!(invalid(with("agent/config-get", &[])));
        assert!(invalid(with("agent/enable", &[("key", "enabled")])));
        assert!(invalid(with(
            "agent/config-set",
            &[("key", "consent"), ("value", "a\nb")]
        )));
        assert!(invalid(with(
            "agent/config-set",
            &[("key", "consent"), ("value", "x"), ("cmd", "y")]
        )));
        let long = "x".repeat(MAX_VALUE + 1);
        assert!(invalid(with(
            "agent/config-set",
            &[("key", "consent"), ("value", &long)]
        )));
    }

    #[test]
    fn access_routes_need_the_protocol_version_too() {
        for manage in examples() {
            let mut args = RouteArgs::default();
            manage.write_args(&mut args);
            assert!(matches!(
                Route::parse(manage.route_name(), &args),
                Err(Error::InvalidArgs { .. })
            ));
        }
    }

    #[test]
    fn no_link_reaches_an_access_route() {
        for manage in examples() {
            let url = format!("easyshot://{}", manage.route_name());
            assert!(matches!(
                Route::from_url(&url),
                Err(Error::UrlNotAllowed(_))
            ));
        }
    }
}
