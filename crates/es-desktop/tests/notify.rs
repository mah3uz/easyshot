mod dbus_support;
mod desktop_support;

use dbus_support::run;
use desktop_support::{QuietBus, serve_on};
use es_desktop::{DesktopNotifier, Notification, Notifier, Urgency};
use std::collections::HashMap;
use std::time::Duration;
use zbus::zvariant::OwnedValue;

#[derive(Debug)]
struct Received {
    summary: String,
    body: String,
    actions: Vec<String>,
    urgency: u8,
    timeout: i32,
}

struct Fake {
    tx: async_channel::Sender<Received>,
    capabilities: Vec<String>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Fake {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        _app_name: String,
        _replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let urgency = hints
            .get("urgency")
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(255);
        let _ = self
            .tx
            .send(Received {
                summary,
                body,
                actions,
                urgency,
                timeout: expire_timeout,
            })
            .await;
        7
    }

    fn get_capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }
}

async fn fake(
    bus: &QuietBus,
    capabilities: &[&str],
) -> (zbus::Connection, async_channel::Receiver<Received>) {
    let (tx, rx) = async_channel::unbounded();
    let fake = Fake {
        tx,
        capabilities: capabilities.iter().map(|c| (*c).to_owned()).collect(),
    };
    let conn = serve_on(
        &bus.address,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        fake,
    )
    .await;
    (conn, rx)
}

fn notification() -> Notification {
    Notification {
        title: "Saved".into(),
        body: "Screenshot saved".into(),
        icon: None,
        image: None,
        actions: vec![
            ("open".into(), "Open".into()),
            ("show".into(), "Show".into()),
        ],
        urgency: Urgency::Critical,
        timeout: Some(Duration::from_secs(5)),
    }
}

async fn emit(conn: &zbus::Connection, id: u32, action: &str) {
    conn.emit_signal(
        None::<&str>,
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        "ActionInvoked",
        &(id, action),
    )
    .await
    .expect("emit");
}

#[test]
fn a_notification_carries_its_actions_urgency_and_timeout_to_the_server() {
    let bus = QuietBus::start();
    let (_server, received) = run(fake(&bus, &["actions", "body"]));
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");
    assert!(notifier.supports_actions());

    notifier.notify(notification()).expect("notify");
    let got = run(received.recv()).expect("received");
    assert_eq!(got.summary, "Saved");
    assert_eq!(got.body, "Screenshot saved");
    assert_eq!(got.actions, ["open", "Open", "show", "Show"]);
    assert_eq!(got.urgency, 2);
    assert_eq!(got.timeout, 5000);
}

#[test]
fn an_invoked_action_reaches_the_channel_and_other_apps_actions_do_not() {
    let bus = QuietBus::start();
    let (server, received) = run(fake(&bus, &["actions"]));
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");

    let id = notifier.notify(notification()).expect("notify");
    assert_eq!(id.0, 7);
    run(received.recv()).expect("received");

    run(emit(&server, 99, "foreign"));
    run(emit(&server, 7, "show"));
    let (got_id, action) = run(notifier.actions().recv()).expect("action");
    assert_eq!((got_id, action.as_str()), (id, "show"));
}

#[test]
fn a_server_without_actions_gets_none_and_the_notifier_says_so() {
    let bus = QuietBus::start();
    let (_server, received) = run(fake(&bus, &["body"]));
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");
    assert!(!notifier.supports_actions());

    notifier.notify(notification()).expect("notify");
    assert!(run(received.recv()).expect("received").actions.is_empty());
}
