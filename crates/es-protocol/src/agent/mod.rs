mod access;
mod capture;
mod reply;
mod target;

pub use access::{AccessStatus, AgentKey, ConfigEntry, GrantSpan, GrantStatus, ManageRoute};
pub use capture::{CaptureSpec, MAX_DELAY_SECONDS, check_out};
pub use reply::{
    AgentReply, CaptureInfo, ErrorCode, OutputEntry, ReplyError, ReplyKind, WindowEntry,
};
pub use target::{AgentTarget, Geometry, OutputName, WindowSelector};

use crate::{ArgValue, Error, Result, RouteArgs};

/// Raised whenever an agent route's arguments change meaning, so a command and a daemon from
/// different releases refuse each other instead of misreading the request.
pub const AGENT_PROTOCOL: i64 = 1;
pub const VERSION_ARG: &str = "version";
pub(crate) const PREFIX: &str = "agent/";

/// Never reachable from the `easyshot://` scheme; only `Route::parse` builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRoute {
    ListOutputs,
    ListWindows,
    Capture(CaptureSpec),
    Manage(ManageRoute),
}

impl AgentRoute {
    /// The routes access gates, and the only ones; `Manage` routes are outside it.
    /// The capture entry is a sample; its fields are not part of the route's identity.
    pub const ALL: [AgentRoute; 3] = [
        AgentRoute::ListOutputs,
        AgentRoute::ListWindows,
        AgentRoute::Capture(CaptureSpec::new(AgentTarget::Fullscreen)),
    ];

    pub fn route_name(&self) -> &'static str {
        match self {
            AgentRoute::ListOutputs => "agent/list-outputs",
            AgentRoute::ListWindows => "agent/list-windows",
            AgentRoute::Capture(_) => "agent/capture",
            AgentRoute::Manage(m) => m.route_name(),
        }
    }

    pub fn kind(&self) -> ReplyKind {
        match self {
            AgentRoute::ListOutputs => ReplyKind::Outputs,
            AgentRoute::ListWindows => ReplyKind::Windows,
            AgentRoute::Capture(_) => ReplyKind::Capture,
            AgentRoute::Manage(m) => m.kind(),
        }
    }

    pub fn command_args(&self) -> Vec<&'static str> {
        match self {
            AgentRoute::ListOutputs => vec!["agent", "list", "outputs"],
            AgentRoute::ListWindows => vec!["agent", "list", "windows"],
            AgentRoute::Capture(_) => vec!["agent", "capture"],
            AgentRoute::Manage(m) => m.command_args(),
        }
    }

    pub fn args(&self) -> RouteArgs {
        let mut args = RouteArgs::default();
        args.0
            .insert(VERSION_ARG.to_owned(), ArgValue::Int(AGENT_PROTOCOL));
        if let AgentRoute::Capture(spec) = self {
            spec.write_args(&mut args);
        }
        if let AgentRoute::Manage(m) = self {
            m.write_args(&mut args);
        }
        args
    }

    /// What the route would capture, for the access scope to check.
    pub fn target(&self) -> Option<&AgentTarget> {
        match self {
            AgentRoute::Capture(spec) => Some(&spec.target),
            AgentRoute::ListOutputs | AgentRoute::ListWindows | AgentRoute::Manage(_) => None,
        }
    }

    pub(crate) fn parse(name: &str, args: &RouteArgs) -> Result<AgentRoute> {
        let manage = ManageRoute::is_name(name);
        let route = Self::ALL.into_iter().find(|r| r.route_name() == name);
        if route.is_none() && !manage {
            return Err(Error::UnknownRoute(name.to_owned()));
        }
        let bad = |reason: String| Error::InvalidArgs {
            route: name.to_owned(),
            reason,
        };
        match args.0.get(VERSION_ARG) {
            Some(ArgValue::Int(AGENT_PROTOCOL)) => {}
            Some(ArgValue::Int(v)) => {
                return Err(bad(format!(
                    "agent protocol {v} is not {AGENT_PROTOCOL}; restart the daemon so it matches this command"
                )));
            }
            _ => return Err(bad(format!("{VERSION_ARG} must be a whole number"))),
        }
        if manage {
            return ManageRoute::parse(name, args)
                .map(AgentRoute::Manage)
                .map_err(bad);
        }
        if let Some(AgentRoute::Capture(_)) = route {
            let known = |k: &str| {
                k == VERSION_ARG || AgentTarget::ARGS.contains(&k) || CaptureSpec::ARGS.contains(&k)
            };
            if let Some(extra) = args.0.keys().find(|k| !known(k)) {
                return Err(bad(format!("unknown argument {extra}")));
            }
            return CaptureSpec::from_args(args)
                .map(AgentRoute::Capture)
                .map_err(bad);
        }
        if let Some(extra) = args.0.keys().find(|k| *k != VERSION_ARG) {
            return Err(bad(format!("unknown argument {extra}")));
        }
        route.ok_or_else(|| Error::UnknownRoute(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Route;

    #[test]
    fn every_agent_route_round_trips_through_its_name_and_args() {
        for agent in AgentRoute::ALL {
            let route = Route::Agent(agent.clone());
            assert_eq!(route.as_str(), agent.route_name());
            assert_eq!(Route::parse(&route.as_str(), &route.args()).unwrap(), route);
            assert_eq!(route.command_args(), agent.command_args());
        }
    }

    #[test]
    fn a_missing_or_different_protocol_version_is_refused() {
        for agent in AgentRoute::ALL {
            let name = agent.route_name();
            assert!(matches!(
                Route::parse(name, &RouteArgs::default()),
                Err(Error::InvalidArgs { .. })
            ));
            for wrong in [
                ArgValue::Int(AGENT_PROTOCOL + 1),
                ArgValue::Int(0),
                ArgValue::Str("1".into()),
                ArgValue::Bool(true),
            ] {
                let mut args = RouteArgs::default();
                args.0.insert(VERSION_ARG.into(), wrong.clone());
                assert!(
                    matches!(Route::parse(name, &args), Err(Error::InvalidArgs { .. })),
                    "{name} {wrong:?}"
                );
            }
        }
    }

    #[test]
    fn list_routes_take_no_other_argument() {
        for agent in [AgentRoute::ListOutputs, AgentRoute::ListWindows] {
            let mut args = agent.args();
            args.0.insert("output".into(), ArgValue::Str("DP-1".into()));
            assert!(matches!(
                Route::parse(agent.route_name(), &args),
                Err(Error::InvalidArgs { .. })
            ));
        }
    }

    #[test]
    fn a_capture_route_carries_its_spec_and_refuses_what_it_does_not_know() {
        let spec = CaptureSpec {
            target: AgentTarget::Window(WindowSelector::any("foot").unwrap()),
            out: Some("/tmp/a.png".into()),
            delay: 5,
            no_ui: true,
        };
        let route = Route::Agent(AgentRoute::Capture(spec));
        assert_eq!(Route::parse(&route.as_str(), &route.args()).unwrap(), route);
        let mut args = route.args();
        args.0.insert("shell".into(), ArgValue::Str("rm".into()));
        assert!(matches!(
            Route::parse("agent/capture", &args),
            Err(Error::InvalidArgs { .. })
        ));
        let mut none = AgentRoute::Capture(CaptureSpec::new(AgentTarget::Fullscreen)).args();
        none.0.remove("fullscreen");
        assert!(matches!(
            Route::parse("agent/capture", &none),
            Err(Error::InvalidArgs { .. })
        ));
    }

    #[test]
    fn unknown_agent_routes_are_unknown_routes() {
        let args = AgentRoute::ListOutputs.args();
        for name in [
            "agent",
            "agent/",
            "agent/list",
            "agent/list-outputs/",
            "agent/List-Outputs",
            "agent/exec",
            "agent/../quit",
        ] {
            assert!(
                matches!(Route::parse(name, &args), Err(Error::UnknownRoute(_))),
                "{name}"
            );
        }
    }

    #[test]
    fn no_link_reaches_an_agent_route() {
        for agent in AgentRoute::ALL {
            for url in [
                format!("easyshot://{}", agent.route_name()),
                format!("EASYSHOT://{}", agent.route_name().to_uppercase()),
                format!("easyshot://{}?version=1", agent.route_name()),
                "easyshot://agent".to_owned(),
            ] {
                assert!(
                    matches!(Route::from_url(&url), Err(Error::UrlNotAllowed(_))),
                    "{url}"
                );
            }
        }
        assert!(
            Route::url_routes().all(|u| !u.contains("agent")),
            "the link allowlist names no agent route"
        );
    }
}
