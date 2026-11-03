#[path = "agent_settings.rs"]
mod agent_settings;
#[path = "agent_scope.rs"]
pub mod scope;

use super::Sources;
use es_app::agent_access::{AgentAccess, AgentAccessHandle, ConsentRequest, Grant, system_clock};
use es_app::events::Origin;
use es_app::jobs::{JobId, run_cpu};
use es_app::services::Services;
use es_app::settings_model::SettingsModel;
use es_desktop::dbus::Caller;
use es_model::{ActionRecord, AgentCfg};
use es_protocol::AgentRoute;
use es_protocol::agent::{AccessStatus, AgentReply, ConfigEntry, ErrorCode, ManageRoute};
use gpui::{App, AppContext as _, Entity, Global, Task};
use scope::{Scope, describe};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Filled by the bus bridge for each agent command and taken by the gate, keyed by the app's job.
pub type Callers = Rc<RefCell<HashMap<JobId, Caller>>>;

const TURN_ON: &str =
    "agent access is off. Turn it on with `easyshot agent enable` or in Settings › Agents.";
const REFUSED: &str = "agent access was refused. Allow it when easyshot asks, or change the \
     consent setting with `easyshot agent config set consent` or in Settings › Agents.";

/// A global so the bus bridge can hand it the callers it reads off the bus.
#[derive(Clone)]
pub struct Gate {
    access: Entity<AgentAccess>,
    settings: Entity<SettingsModel>,
    callers: Callers,
    uid: u32,
}

/// The work an admitted route does; it may run on any executor.
pub type Work = Box<dyn FnOnce(&AgentRoute, &Scope, &mut App) -> Task<AgentReply>>;

impl Global for Gate {}

pub fn install(cx: &mut App) -> Gate {
    let services = cx.global::<Services>();
    let (settings, paths) = (services.settings.clone(), services.stores.paths.clone());
    let file = es_store::agent::open(&paths);
    let access = cx.new(|_| AgentAccess::new(file, system_clock()));
    cx.set_global(AgentAccessHandle(access.clone()));
    let gate = Gate::new(access, settings, rustix::process::getuid().as_raw());
    cx.set_global(gate.clone());
    gate
}

fn comm(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_owned())
}

impl Gate {
    pub fn new(access: Entity<AgentAccess>, settings: Entity<SettingsModel>, uid: u32) -> Gate {
        Gate {
            access,
            settings,
            callers: Callers::default(),
            uid,
        }
    }

    pub fn callers(&self) -> Callers {
        self.callers.clone()
    }

    fn cfg(&self, cx: &App) -> AgentCfg {
        self.settings.read(cx).config().agent.clone()
    }

    /// Every agent route passes here, and nowhere else, before any of its work starts.
    pub fn pass(
        &self,
        job: JobId,
        route: AgentRoute,
        origin: Origin,
        sources: &Sources,
        work: Work,
        cx: &mut App,
    ) -> Task<AgentReply> {
        let caller = self.callers.borrow_mut().remove(&job).unwrap_or_default();
        let comm = caller.pid.and_then(comm);
        let kind = route.kind();
        let mut log = ActionRecord {
            at: self.access.read(cx).now(),
            route: route.route_name().to_owned(),
            target: route.target().map(describe),
            result: String::new(),
            pid: caller.pid,
            comm: comm.clone(),
        };
        let refused = move |code, message: &str| AgentReply::failed(kind, code, message);
        let early = if origin != Origin::DBus {
            Some(refused(
                ErrorCode::AccessDenied,
                "agent commands are accepted over D-Bus only",
            ))
        } else if caller.uid != Some(self.uid) {
            Some(refused(
                ErrorCode::AccessDenied,
                "agent commands are accepted from your own user only",
            ))
        } else {
            None
        };
        if let Some(reply) = early {
            self.record(log, &reply, cx);
            return Task::ready(reply);
        }
        if let AgentRoute::Manage(m) = &route {
            let reply = self.manage(m, cx);
            if let ManageRoute::ConfigSet(key, value) = m {
                log.target = Some(format!("{}={value}", key.name()));
            }
            if !matches!(m, ManageRoute::Status | ManageRoute::ConfigGet(_)) {
                self.record(log, &reply, cx);
            }
            return Task::ready(reply);
        }
        let cfg = self.cfg(cx);
        if !cfg.enabled {
            let reply = refused(ErrorCode::AccessOff, TURN_ON);
            self.record(log, &reply, cx);
            return Task::ready(reply);
        }
        let scope = Scope::new(&cfg);
        let checked = match route.target().filter(|_| !scope.is_open()).cloned() {
            Some(target) => {
                let (scope, sources) = (scope.clone(), sources.clone());
                run_cpu(cx, move || {
                    scope.check(&target, &(sources.outputs)(), &(sources.windows)())
                })
            }
            None => Task::ready(Ok(Ok(()))),
        };
        let request = ConsentRequest {
            route: route.route_name(),
            target: log.target.clone(),
            pid: caller.pid,
            comm,
            remember: true,
        };
        let gate = self.clone();
        cx.spawn(async move |cx| {
            let reply = match checked.await {
                Ok(Ok(())) => {
                    let asked = cx
                        .update(|cx| AgentAccess::consent(&gate.access, cfg.consent, request, cx));
                    let allowed = asked.await;
                    let still_on = cx.update(|cx| gate.cfg(cx).enabled);
                    if !still_on {
                        refused(ErrorCode::AccessOff, TURN_ON)
                    } else if !allowed {
                        refused(ErrorCode::AccessDenied, REFUSED)
                    } else {
                        let mut reply = cx.update(|cx| work(&route, &scope, cx)).await;
                        scope.filter(&mut reply);
                        reply
                    }
                }
                Ok(Err(outside)) => refused(ErrorCode::AccessDenied, &outside),
                Err(e) => refused(ErrorCode::Failed, &e.to_string()),
            };
            cx.update(|cx| gate.record(log, &reply, cx));
            reply
        })
    }

    fn record(&self, mut entry: ActionRecord, reply: &AgentReply, cx: &mut App) {
        entry.result = match &reply.error {
            Some(e) => e.code.as_str().to_owned(),
            None if reply.ok => "ok".to_owned(),
            None => ErrorCode::Failed.as_str().to_owned(),
        };
        let limit = self.cfg(cx).log_limit();
        self.access
            .update(cx, |access, cx| access.record(entry, limit, cx));
    }

    fn status(&self, cx: &App) -> AccessStatus {
        let cfg = self.cfg(cx);
        let access = self.access.read(cx);
        AccessStatus {
            enabled: cfg.enabled,
            consent: cfg.consent.name().to_owned(),
            grant: access.grant().map(Grant::status),
            allowed_outputs: cfg.allowed_outputs,
            allowed_windows: cfg.allowed_windows,
            redact_secrets: cfg.redact_secrets,
            recent_actions: access.log().len(),
        }
    }

    fn manage(&self, route: &ManageRoute, cx: &mut App) -> AgentReply {
        let kind = route.kind();
        let mut next = self.cfg(cx);
        let key = match route {
            ManageRoute::Enable => {
                next.enabled = true;
                None
            }
            ManageRoute::Disable => {
                next.enabled = false;
                None
            }
            ManageRoute::Status => None,
            ManageRoute::ConfigGet(key) => Some(*key),
            ManageRoute::ConfigSet(key, value) => {
                if let Err(e) = agent_settings::set(&mut next, *key, value) {
                    return AgentReply::failed(kind, ErrorCode::InvalidArguments, e);
                }
                Some(*key)
            }
        };
        if !next.enabled {
            self.access.update(cx, |access, cx| access.revoke(cx));
        }
        if next != self.cfg(cx) {
            self.settings.update(cx, |settings, cx| {
                settings.update(|c| c.agent = next.clone(), cx)
            });
        }
        match key {
            Some(key) => AgentReply {
                config: Some(ConfigEntry {
                    key: key.name().to_owned(),
                    value: agent_settings::get(&next, key),
                }),
                ..AgentReply::ok(kind)
            },
            None => AgentReply {
                access: Some(self.status(cx)),
                ..AgentReply::ok(kind)
            },
        }
    }
}

#[cfg(test)]
#[path = "agent_gate_tests.rs"]
pub(in crate::daemon) mod tests;
