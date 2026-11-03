use super::tests::{BUS_PID, GateRig, JOB, ME, sources};
use super::*;
use crate::daemon::agent::handler;
use es_app::jobs::Jobs;
use es_model::Consent;
use es_protocol::JobStatus;
use gpui::TestAppContext;
use serde_json::json;

fn manage(m: ManageRoute) -> AgentRoute {
    AgentRoute::Manage(m)
}

fn enabled(rig: &GateRig, cx: &TestAppContext) -> bool {
    cx.read(|cx| rig.settings.read(cx).config().agent.enabled)
}

#[gpui::test]
fn agent_enable_waits_for_the_click_and_cancel_exits_eight(cx: &mut TestAppContext) {
    let rig = GateRig::new(AgentCfg::default(), cx);
    rig.prompt.hold.set(true);
    let src = sources();
    let (jobs, finished) = Jobs::new();
    rig.gate.callers().borrow_mut().insert(
        JOB,
        Caller {
            uid: Some(ME),
            pid: Some(BUS_PID),
        },
    );
    let handler = handler(src.sources.clone(), jobs, rig.gate.clone());
    cx.update(|cx| handler(JOB, manage(ManageRoute::Enable), Origin::DBus, cx));
    cx.run_until_parked();
    assert!(
        finished.try_recv().is_err(),
        "the command waits for the click"
    );
    assert!(!enabled(&rig, cx), "nothing changes before the click");
    let asked = rig.prompt.confirmed.borrow().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].change, Loosening::TurnOn);
    assert_eq!(asked[0].pid, Some(BUS_PID));

    let click = rig.prompt.held.borrow_mut().pop().unwrap();
    click.send(false).unwrap();
    cx.run_until_parked();
    let done = finished.try_recv().expect("cancel answers the command");
    assert_eq!(done.status, JobStatus::Error);
    let reply = AgentReply::from_json(done.detail.as_deref().unwrap()).unwrap();
    assert_eq!(reply.exit_code(), 8);
    assert!(!enabled(&rig, cx), "cancel leaves access off");
    assert_eq!(
        rig.log(cx)[0],
        ("agent/enable".to_owned(), "access_denied".to_owned())
    );
}

#[gpui::test]
fn agent_enable_turns_access_on_once_allowed(cx: &mut TestAppContext) {
    let rig = GateRig::new(AgentCfg::default(), cx);
    let src = sources();
    let (status, reply) = rig.ask(
        &src.sources,
        manage(ManageRoute::Enable),
        Origin::DBus,
        Some(ME),
        cx,
    );
    assert_eq!(status, JobStatus::Ok);
    assert_eq!(reply["access"]["enabled"], json!(true));
    assert!(enabled(&rig, cx));
}

#[gpui::test]
fn only_changes_that_give_agents_more_ask_first(cx: &mut TestAppContext) {
    let rig = GateRig::new(
        AgentCfg {
            enabled: true,
            allowed_outputs: vec!["DP-1".into()],
            allowed_windows: vec!["firefox".into()],
            ..AgentCfg::default()
        },
        cx,
    );
    rig.prompt.confirms.set(false);
    let src = sources();
    let set = |key, value: &str| manage(ManageRoute::ConfigSet(key, value.into()));
    let wider = Some(Loosening::WiderScope);
    let cases = [
        (
            set(AgentKey::Consent, "always-allow"),
            Some(Loosening::AlwaysAllow),
        ),
        (set(AgentKey::AllowedOutputs, "DP-1,DP-2"), wider),
        (set(AgentKey::AllowedOutputs, ""), wider),
        (set(AgentKey::AllowedWindows, ""), wider),
        (set(AgentKey::AllowedWindows, "firefox,kitty"), wider),
        (set(AgentKey::Consent, "always-ask"), None),
        (set(AgentKey::LogLimit, "20"), None),
        (manage(ManageRoute::Disable), None),
    ];
    for (route, expected) in cases {
        rig.prompt.confirmed.borrow_mut().clear();
        let (_, reply) = rig.ask(&src.sources, route.clone(), Origin::DBus, Some(ME), cx);
        let asked = rig.prompt.confirmed.borrow().first().map(|r| r.change);
        assert_eq!(asked, expected, "{route:?}");
        assert_eq!(reply["ok"], json!(expected.is_none()), "{route:?}");
    }
    let cfg = cx.read(|cx| rig.settings.read(cx).config().agent.clone());
    assert_eq!(cfg.consent, Consent::AlwaysAsk);
    assert_eq!(
        cfg.allowed_outputs,
        ["DP-1"],
        "a cancelled widening changes nothing"
    );
    assert!(!cfg.enabled);
}
