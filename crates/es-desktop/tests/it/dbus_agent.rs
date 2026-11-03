use crate::dbus_support;

use dbus_support::{PrivateBus, run, service};
use es_desktop::dbus::{Client, ClientError, Incoming, Launch};
use es_protocol::agent::{AGENT_PROTOCOL, AgentReply, ReplyKind, VERSION_ARG};
use es_protocol::{AgentRoute, ArgValue, JobStatus, Route, RouteArgs};

async fn connect(bus: &PrivateBus) -> Client {
    Client::connect(&bus.endpoint(), Launch::default())
        .await
        .expect("connect")
}

#[test]
fn an_agent_route_crosses_the_bus_typed_and_its_json_answer_comes_back_whole() {
    run(async {
        let bus = PrivateBus::start();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;
        let agent = AgentRoute::ListWindows;
        let reply = AgentReply {
            windows: Some(Vec::new()),
            ..AgentReply::ok(ReplyKind::Windows)
        };
        let json = reply.to_json();
        let args = agent.args();

        let asked = client.run(agent.route_name(), &args, futures::future::pending());
        let app = async {
            let Incoming::Command { job, route, .. } = incoming.recv().await.expect("command")
            else {
                panic!("expected a command");
            };
            assert_eq!(route, Route::Agent(AgentRoute::ListWindows));
            svc.finish(job, JobStatus::Ok, &json).await.expect("finish");
        };
        let (outcome, ()) = futures::join!(asked, app);
        let outcome = outcome.expect("outcome");
        assert_eq!(outcome.status, JobStatus::Ok);
        assert_eq!(AgentReply::from_json(&outcome.result).expect("json"), reply);
    });
}

#[test]
fn agent_routes_with_the_wrong_version_or_extra_arguments_never_reach_the_app() {
    run(async {
        let bus = PrivateBus::start();
        let svc = service(&bus.endpoint()).await;
        let client = connect(&bus).await;
        let name = AgentRoute::ListOutputs.route_name();
        let mut newer = AgentRoute::ListOutputs.args();
        newer
            .0
            .insert(VERSION_ARG.into(), ArgValue::Int(AGENT_PROTOCOL + 1));
        let mut extra = AgentRoute::ListOutputs.args();
        extra
            .0
            .insert("cmd".into(), ArgValue::Str("rm -rf ~".into()));
        for args in [RouteArgs::default(), newer, extra] {
            let refused = client.submit(name, &args).await;
            assert!(
                matches!(refused, Err(ClientError::Rejected(_))),
                "{args:?}: {refused:?}"
            );
        }
        assert!(svc.incoming().try_recv().is_err());
    });
}

#[test]
fn submit_returns_the_job_without_waiting_for_it() {
    run(async {
        let bus = PrivateBus::start();
        let svc = service(&bus.endpoint()).await;
        let client = connect(&bus).await;
        let agent = AgentRoute::ListOutputs;
        let job = client
            .submit(agent.route_name(), &agent.args())
            .await
            .expect("accepted");
        let Ok(Incoming::Command {
            job: seen, route, ..
        }) = svc.incoming().recv().await
        else {
            panic!("expected a command");
        };
        assert_eq!((seen, route), (job, Route::Agent(agent)));
    });
}

#[test]
fn an_agent_route_arrives_with_the_sender_credentials_the_bus_vouches_for() {
    use std::os::unix::fs::MetadataExt as _;
    run(async {
        let bus = PrivateBus::start();
        let svc = service(&bus.endpoint()).await;
        let client = connect(&bus).await;
        let agent = AgentRoute::ListOutputs;
        client
            .submit(agent.route_name(), &agent.args())
            .await
            .expect("accepted");
        let Ok(Incoming::Command { caller, .. }) = svc.incoming().recv().await else {
            panic!("expected a command");
        };
        let me = std::fs::metadata("/proc/self").expect("proc").uid();
        let caller = caller.expect("an agent route carries its caller");
        assert_eq!(caller.uid, Some(me));
        assert_eq!(caller.pid, Some(std::process::id()));

        client
            .submit("deck/toggle", &RouteArgs::default())
            .await
            .expect("accepted");
        let Ok(Incoming::Command { caller, .. }) = svc.incoming().recv().await else {
            panic!("expected a command");
        };
        assert_eq!(caller, None, "other routes cost no extra bus call");
    });
}
