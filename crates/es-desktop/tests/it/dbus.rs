use crate::dbus_support;

use dbus_support::{PrivateBus, callbacks, run, service};
use es_desktop::dbus::{Client, ClientError, Incoming, Launch, Service, ServiceError};
use es_protocol::{ArgValue, JobStatus, Route, RouteArgs};
use std::time::Duration;

fn never() -> impl std::future::Future<Output = ()> {
    futures::future::pending()
}

async fn connect(bus: &PrivateBus) -> Client {
    Client::connect(&bus.endpoint(), Launch::default())
        .await
        .expect("connect")
}

#[test]
fn a_second_service_cannot_take_the_name() {
    run(async {
        let bus = PrivateBus::start();
        let _first = service(&bus.endpoint()).await;
        let second = Service::start(bus.endpoint(), callbacks()).await;
        assert!(matches!(second, Err(ServiceError::AlreadyRunning)));
    });
}

#[test]
fn a_valid_command_returns_a_job_and_reaches_the_app() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;

        let run = client.run("capture/region", &no_args, never());
        let app = async {
            let Incoming::Command { job, route } = incoming.recv().await.expect("command") else {
                panic!("expected a command");
            };
            assert_eq!(route, Route::CaptureRegion);
            svc.finish(job, JobStatus::Ok, "done")
                .await
                .expect("finish");
        };
        let (outcome, ()) = futures::join!(run, app);
        let outcome = outcome.expect("outcome");
        assert_eq!(outcome.status, JobStatus::Ok);
        assert_eq!(outcome.result, "done");
    });
}

#[test]
fn unknown_routes_and_stray_arguments_are_rejected_before_the_app_sees_them() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let client = connect(&bus).await;

        let unknown = client.run("rm-rf", &no_args, never()).await;
        assert!(
            matches!(unknown, Err(ClientError::Rejected(_))),
            "{unknown:?}"
        );

        let mut args = RouteArgs::default();
        args.0.insert("x".into(), ArgValue::Bool(true));
        let stray = client.run("capture/region", &args, never()).await;
        assert!(matches!(stray, Err(ClientError::Rejected(_))), "{stray:?}");

        assert!(svc.incoming().try_recv().is_err());
    });
}

#[test]
fn cancel_reaches_the_app_and_the_job_ends_cancelled() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;

        let (tx, rx) = async_channel::bounded::<()>(1);
        let ctrl_c = async move {
            let _ = rx.recv().await;
        };
        let run = client.run("record", &no_args, ctrl_c);
        let app = async {
            let Incoming::Command { job, .. } = incoming.recv().await.expect("command") else {
                panic!("expected a command");
            };
            tx.send(()).await.expect("ctrl-c");
            let Incoming::Cancel { job: cancelled } = incoming.recv().await.expect("cancel") else {
                panic!("expected a cancel");
            };
            assert_eq!(cancelled, job);
            svc.finish(job, JobStatus::Cancelled, "")
                .await
                .expect("finish");
        };
        let (outcome, ()) = futures::join!(run, app);
        assert_eq!(outcome.expect("outcome").status, JobStatus::Cancelled);
    });
}

#[test]
fn each_client_gets_the_result_of_its_own_job() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client_a = connect(&bus).await;
        let client_b = connect(&bus).await;

        let first = client_a.run("gallery", &no_args, never());
        let second = client_b.run("ocr", &no_args, never());
        let app = async {
            let mut jobs = Vec::new();
            for _ in 0..2 {
                let Incoming::Command { job, route } = incoming.recv().await.expect("command")
                else {
                    panic!("expected a command");
                };
                jobs.push((job, route));
            }
            for (job, route) in jobs.into_iter().rev() {
                svc.finish(job, JobStatus::Error, &route.to_string())
                    .await
                    .expect("finish");
            }
        };
        let (a, b, ()) = futures::join!(first, second, app);
        assert_eq!(a.expect("a").result, "gallery");
        assert_eq!(b.expect("b").result, "ocr");
    });
}

#[test]
fn the_client_exits_with_code_3_when_the_daemon_vanishes() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;

        let run = client.run("record", &no_args, never());
        let app = async {
            incoming.recv().await.expect("command");
            svc.close().await;
        };
        let (outcome, ()) = futures::join!(run, app);
        let err = outcome.expect_err("daemon is gone");
        assert!(matches!(err, ClientError::DaemonGone));
        assert_eq!(err.exit_code(), 3);
    });
}

#[test]
fn doctor_returns_what_the_app_supplies() {
    run(async {
        let bus = PrivateBus::start();
        let _svc = service(&bus.endpoint()).await;
        let client = connect(&bus).await;
        assert_eq!(client.doctor().await.expect("doctor"), r#"{"ok":true}"#);
    });
}

#[test]
fn connecting_without_a_daemon_gives_up_after_the_wait() {
    run(async {
        let bus = PrivateBus::start();
        let launch = Launch {
            spawn: None,
            wait: Duration::from_millis(300),
        };
        let err = Client::connect(&bus.endpoint(), launch).await.err();
        assert!(matches!(err, Some(ClientError::DaemonUnavailable)));
    });
}

#[test]
fn the_test_interface_is_not_served_on_the_real_name() {
    run(async {
        let bus = PrivateBus::start();
        let _svc = service(&bus.endpoint()).await;
        let conn = zbus::connection::Builder::address(bus.address.as_str())
            .expect("address")
            .build()
            .await
            .expect("connect");
        let reply = conn
            .call_method(
                Some(es_protocol::BUS_NAME),
                es_protocol::OBJECT_PATH,
                Some(es_protocol::TEST_INTERFACE),
                "Dump",
                &(),
            )
            .await;
        assert!(reply.is_err());
    });
}

async fn bystander(bus: &PrivateBus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address.as_str())
        .expect("address")
        .build()
        .await
        .expect("connect")
}

async fn pause(ms: u64) {
    let (tx, rx) = async_channel::bounded::<()>(1);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        let _ = tx.send_blocking(());
    });
    let _ = rx.recv().await;
}

#[test]
fn job_results_reach_only_the_caller() {
    run(async {
        use futures::{FutureExt, StreamExt};
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;
        let other = bystander(&bus).await;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(es_protocol::INTERFACE)
            .expect("interface")
            .member("JobFinished")
            .expect("member")
            .build();
        let mut overheard = zbus::MessageStream::for_match_rule(rule, &other, None)
            .await
            .expect("match")
            .fuse();

        let run = client.run("gallery", &no_args, never());
        let app = async {
            let Incoming::Command { job, .. } = incoming.recv().await.expect("command") else {
                panic!("expected a command");
            };
            svc.finish(job, JobStatus::Error, "/home/someone/private.png")
                .await
                .expect("finish");
        };
        let (outcome, ()) = futures::join!(run, app);
        assert_eq!(
            outcome.expect("outcome").result,
            "/home/someone/private.png"
        );
        futures::select! {
            msg = overheard.next() => panic!("a bystander saw the result: {msg:?}"),
            () = pause(300).fuse() => {}
        }
    });
}

#[test]
fn only_the_caller_can_cancel_its_job() {
    run(async {
        let bus = PrivateBus::start();
        let no_args = RouteArgs::default();
        let svc = service(&bus.endpoint()).await;
        let incoming = svc.incoming();
        let client = connect(&bus).await;
        let other = bystander(&bus).await;

        let run = client.run("record", &no_args, never());
        let app = async {
            let Incoming::Command { job, .. } = incoming.recv().await.expect("command") else {
                panic!("expected a command");
            };
            let reply = other
                .call_method(
                    Some(es_protocol::BUS_NAME),
                    es_protocol::OBJECT_PATH,
                    Some(es_protocol::INTERFACE),
                    "Cancel",
                    &(job,),
                )
                .await;
            assert!(reply.is_err(), "a stranger cancelled the job");
            assert!(incoming.try_recv().is_err());
            svc.finish(job, JobStatus::Ok, "").await.expect("finish");
        };
        let (outcome, ()) = futures::join!(run, app);
        assert_eq!(outcome.expect("outcome").status, JobStatus::Ok);
    });
}
