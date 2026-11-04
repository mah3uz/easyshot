use super::*;
use crate::daemon::agent::handler;
use es_app::agent_access::{Answer, ConsentPrompt};
use es_app::jobs::Jobs;
use es_backend::Caps;
use es_geom::Rect;
use es_model::Consent;
use es_model::platform::{OutputId, OutputInfo, Transform};
use es_protocol::agent::{AgentKey, GrantSpan};
use es_store::{ConfigFile, Paths};
use gpui::TestAppContext;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use time::OffsetDateTime;

pub(in crate::daemon) const ME: u32 = 1000;
pub(in crate::daemon) const JOB: JobId = JobId(7);
pub(in crate::daemon) const BUS_PID: u32 = 4242;

pub(in crate::daemon) struct FakePrompt {
    answer: Cell<Answer>,
    asked: Cell<usize>,
    pub confirms: Cell<bool>,
    pub hold: Cell<bool>,
    pub held: RefCell<Vec<futures::channel::oneshot::Sender<bool>>>,
    pub confirmed: RefCell<Vec<ChangeRequest>>,
}

impl ConsentPrompt for FakePrompt {
    fn ask(&self, _: ConsentRequest, _: &mut App) -> Task<Answer> {
        self.asked.set(self.asked.get() + 1);
        Task::ready(self.answer.get())
    }

    fn confirm(&self, request: ChangeRequest, cx: &mut App) -> Task<bool> {
        self.confirmed.borrow_mut().push(request);
        if !self.hold.get() {
            return Task::ready(self.confirms.get());
        }
        let (tx, rx) = futures::channel::oneshot::channel();
        self.held.borrow_mut().push(tx);
        cx.spawn(async move |_| rx.await.unwrap_or(false))
    }
}

pub(in crate::daemon) struct GateRig {
    dir: tempfile::TempDir,
    pub settings: Entity<SettingsModel>,
    pub access: Entity<AgentAccess>,
    pub gate: Gate,
    pub prompt: Rc<FakePrompt>,
}

impl GateRig {
    pub fn new(agent: AgentCfg, cx: &mut TestAppContext) -> GateRig {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let config = ConfigFile::load(&dir.path().join("config.toml")).unwrap();
        let settings = cx.new(|_| SettingsModel::new(config));
        settings.update(cx, |s, cx| s.update(|c| c.agent = agent, cx));
        Self::open(dir, settings, cx)
    }

    /// The same files and settings as a daemon that starts again.
    pub fn restart(self, cx: &mut TestAppContext) -> GateRig {
        Self::open(self.dir, self.settings, cx)
    }

    fn open(
        dir: tempfile::TempDir,
        settings: Entity<SettingsModel>,
        cx: &mut TestAppContext,
    ) -> GateRig {
        let file = es_store::agent::open(&Paths::rooted(dir.path()));
        let prompt = Rc::new(FakePrompt {
            answer: Cell::new(Answer::Deny),
            asked: Cell::new(0),
            confirms: Cell::new(true),
            hold: Cell::new(false),
            held: RefCell::default(),
            confirmed: RefCell::default(),
        });
        let installed: Rc<dyn ConsentPrompt> = prompt.clone();
        let access = cx.new(|_| {
            let mut access = AgentAccess::new(file, Rc::new(|| OffsetDateTime::UNIX_EPOCH));
            access.set_prompt(installed);
            access
        });
        let gate = Gate::new(access.clone(), settings.clone(), ME);
        GateRig {
            dir,
            settings,
            access,
            gate,
            prompt,
        }
    }

    pub fn permissive(cx: &mut TestAppContext) -> GateRig {
        GateRig::new(
            AgentCfg {
                enabled: true,
                consent: Consent::AlwaysAllow,
                ..AgentCfg::default()
            },
            cx,
        )
    }

    pub fn answer(&self, answer: Answer) {
        self.prompt.answer.set(answer);
    }

    pub fn ask(
        &self,
        sources: &Sources,
        route: AgentRoute,
        origin: Origin,
        uid: Option<u32>,
        cx: &mut TestAppContext,
    ) -> (JobStatus, Value) {
        let (jobs, finished) = Jobs::new();
        self.gate.callers().borrow_mut().insert(
            JOB,
            Caller {
                uid,
                pid: Some(BUS_PID),
            },
        );
        let handler = handler(sources.clone(), jobs, self.gate.clone());
        cx.update(|cx| handler(JOB, route, origin, cx));
        cx.run_until_parked();
        let done = finished
            .try_recv()
            .expect("the agent handler finishes its job");
        let reply = serde_json::from_str(done.detail.as_deref().expect("a JSON answer")).unwrap();
        (done.status, reply)
    }

    pub fn log(&self, cx: &TestAppContext) -> Vec<(String, String)> {
        cx.read(|cx| {
            self.access
                .read(cx)
                .log()
                .iter()
                .map(|e| (e.route.clone(), e.result.clone()))
                .collect()
        })
    }
}

use es_protocol::JobStatus;

pub(in crate::daemon) struct Counted {
    pub sources: Sources,
    calls: Arc<AtomicUsize>,
}

pub(in crate::daemon) fn sources() -> Counted {
    let calls = Arc::new(AtomicUsize::new(0));
    let (a, b) = (calls.clone(), calls.clone());
    let output = |name: &str, x: f64| OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical: Rect::new(x, 0., x + 100., 100.),
        physical: (100, 100),
        scale: 1.,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    };
    let outputs = vec![output("DP-1", 0.), output("DP-2", 100.)];
    Counted {
        sources: Sources {
            caps: Caps::all(),
            outputs: Arc::new(move || {
                a.fetch_add(1, Ordering::SeqCst);
                outputs.clone()
            }),
            windows: Arc::new(move || {
                b.fetch_add(1, Ordering::SeqCst);
                Vec::new()
            }),
        },
        calls,
    }
}

fn manage(m: ManageRoute) -> AgentRoute {
    AgentRoute::Manage(m)
}

fn code(reply: &Value) -> &Value {
    &reply["error"]["code"]
}

#[gpui::test]
fn every_agent_route_is_refused_while_access_is_off_and_says_how_to_turn_it_on(
    cx: &mut TestAppContext,
) {
    let rig = GateRig::new(AgentCfg::default(), cx);
    let src = sources();
    for route in AgentRoute::ALL {
        let (status, reply) = rig.ask(&src.sources, route.clone(), Origin::DBus, Some(ME), cx);
        assert_eq!(status, JobStatus::Error);
        assert_eq!(code(&reply), &json!("access_off"), "{route:?}");
        let parsed = AgentReply::from_json(&reply.to_string()).unwrap();
        assert_eq!(parsed.exit_code(), 7);
        assert_eq!(
            reply["error"]["hint"],
            json!(
                "Agent access is off. Turn it on in Settings › Agents, or run \
                 `easyshot agent enable`."
            )
        );
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("easyshot agent enable")
        );
    }
    assert_eq!(src.calls.load(Ordering::SeqCst), 0, "nothing was looked at");
    assert_eq!(rig.prompt.asked.get(), 0, "nobody is asked while it is off");
}

#[gpui::test]
fn every_agent_route_is_refused_when_the_user_denies(cx: &mut TestAppContext) {
    let rig = GateRig::new(
        AgentCfg {
            enabled: true,
            ..AgentCfg::default()
        },
        cx,
    );
    let src = sources();
    for route in AgentRoute::ALL {
        let (_, reply) = rig.ask(&src.sources, route.clone(), Origin::DBus, Some(ME), cx);
        assert_eq!(code(&reply), &json!("access_denied"), "{route:?}");
        assert_eq!(
            reply["error"]["hint"],
            json!("You didn't allow this request, so nothing was done.")
        );
        assert_eq!(
            AgentReply::from_json(&reply.to_string())
                .unwrap()
                .exit_code(),
            8
        );
    }
    assert_eq!(rig.prompt.asked.get(), AgentRoute::ALL.len());
    assert_eq!(src.calls.load(Ordering::SeqCst), 0);
}

#[gpui::test]
fn another_user_or_an_unknown_caller_is_refused_even_with_access_open(cx: &mut TestAppContext) {
    let rig = GateRig::permissive(cx);
    let src = sources();
    for uid in [Some(ME + 1), Some(0), None] {
        let (_, reply) = rig.ask(&src.sources, AgentRoute::ListOutputs, Origin::DBus, uid, cx);
        assert_eq!(code(&reply), &json!("access_denied"), "{uid:?}");
        let (_, reply) = rig.ask(
            &src.sources,
            manage(ManageRoute::Enable),
            Origin::DBus,
            uid,
            cx,
        );
        assert_eq!(
            code(&reply),
            &json!("access_denied"),
            "{uid:?} may not turn access on"
        );
    }
    assert_eq!(src.calls.load(Ordering::SeqCst), 0);
    let (_, reply) = rig.ask(
        &src.sources,
        AgentRoute::ListOutputs,
        Origin::DBus,
        Some(ME),
        cx,
    );
    assert_eq!(reply["ok"], json!(true));
}

#[gpui::test]
fn enable_status_config_and_disable_work_while_off_and_round_trip(cx: &mut TestAppContext) {
    let rig = GateRig::new(AgentCfg::default(), cx);
    let src = sources();
    let mut ask = |route: ManageRoute| {
        rig.ask(&src.sources, manage(route), Origin::DBus, Some(ME), cx)
            .1
    };
    let status = ask(ManageRoute::Status);
    assert_eq!(status["access"]["enabled"], json!(false));
    assert_eq!(status["access"]["consent"], json!("once-per-session"));
    assert_eq!(ask(ManageRoute::Enable)["access"]["enabled"], json!(true));
    let set = ask(ManageRoute::ConfigSet(
        AgentKey::AllowedOutputs,
        "DP-1, DP-1".into(),
    ));
    assert_eq!(
        set["config"],
        json!({"key": "allowed_outputs", "value": "DP-1"})
    );
    let got = ask(ManageRoute::ConfigGet(AgentKey::AllowedOutputs));
    assert_eq!(got["config"]["value"], json!("DP-1"));
    let bad = ask(ManageRoute::ConfigSet(AgentKey::Consent, "never".into()));
    assert_eq!(code(&bad), &json!("invalid_arguments"));
    let status = ask(ManageRoute::Status);
    assert_eq!(status["access"]["allowed_outputs"], json!(["DP-1"]));
    assert_eq!(ask(ManageRoute::Disable)["access"]["enabled"], json!(false));
    let cfg = cx.read(|cx| rig.settings.read(cx).config().agent.clone());
    assert_eq!(
        (cfg.enabled, cfg.allowed_outputs),
        (false, vec!["DP-1".to_owned()])
    );
}

#[gpui::test]
fn turning_access_off_ends_the_grant(cx: &mut TestAppContext) {
    let rig = GateRig::new(
        AgentCfg {
            enabled: true,
            ..AgentCfg::default()
        },
        cx,
    );
    rig.access
        .update(cx, |a, cx| a.allow(GrantSpan::Session, cx));
    let src = sources();
    rig.ask(
        &src.sources,
        manage(ManageRoute::Disable),
        Origin::DBus,
        Some(ME),
        cx,
    );
    assert_eq!(cx.read(|cx| rig.access.read(cx).grant()), None);
}

#[gpui::test]
fn always_survives_a_daemon_restart(cx: &mut TestAppContext) {
    let rig = GateRig::new(
        AgentCfg {
            enabled: true,
            ..AgentCfg::default()
        },
        cx,
    );
    let src = sources();
    rig.answer(Answer::Allow(GrantSpan::Always));
    let (_, reply) = rig.ask(
        &src.sources,
        AgentRoute::ListOutputs,
        Origin::DBus,
        Some(ME),
        cx,
    );
    assert_eq!(reply["ok"], json!(true));
    let rig = rig.restart(cx);
    let (_, reply) = rig.ask(
        &src.sources,
        AgentRoute::ListWindows,
        Origin::DBus,
        Some(ME),
        cx,
    );
    assert_eq!(reply["ok"], json!(true), "{reply}");
    assert_eq!(
        rig.prompt.asked.get(),
        0,
        "not asked again after the restart"
    );
}

#[gpui::test]
fn lists_show_only_the_outputs_in_scope(cx: &mut TestAppContext) {
    let rig = GateRig::new(
        AgentCfg {
            enabled: true,
            consent: Consent::AlwaysAllow,
            allowed_outputs: vec!["DP-1".into()],
            ..AgentCfg::default()
        },
        cx,
    );
    let src = sources();
    let (_, reply) = rig.ask(
        &src.sources,
        AgentRoute::ListOutputs,
        Origin::DBus,
        Some(ME),
        cx,
    );
    let names: Vec<&str> = reply["outputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["DP-1"]);
}

#[gpui::test]
fn the_log_records_allowed_and_refused_calls_with_the_caller(cx: &mut TestAppContext) {
    let rig = GateRig::new(AgentCfg::default(), cx);
    let src = sources();
    rig.ask(
        &src.sources,
        AgentRoute::ListOutputs,
        Origin::DBus,
        Some(ME),
        cx,
    );
    rig.ask(
        &src.sources,
        manage(ManageRoute::Status),
        Origin::DBus,
        Some(ME),
        cx,
    );
    rig.ask(
        &src.sources,
        manage(ManageRoute::Enable),
        Origin::DBus,
        Some(ME),
        cx,
    );
    rig.ask(
        &src.sources,
        AgentRoute::ListOutputs,
        Origin::DBus,
        Some(ME + 1),
        cx,
    );
    rig.answer(Answer::Allow(GrantSpan::Hour));
    rig.ask(
        &src.sources,
        AgentRoute::ListWindows,
        Origin::DBus,
        Some(ME),
        cx,
    );
    let pair = |r: &str, s: &str| (r.to_owned(), s.to_owned());
    assert_eq!(
        rig.log(cx),
        [
            pair("agent/list-windows", "ok"),
            pair("agent/list-outputs", "access_denied"),
            pair("agent/enable", "ok"),
            pair("agent/list-outputs", "access_off"),
        ],
        "newest first; reading the status is not an action"
    );
    let pids = cx.read(|cx| {
        rig.access
            .read(cx)
            .log()
            .iter()
            .all(|e| e.pid == Some(BUS_PID))
    });
    assert!(pids);
}
