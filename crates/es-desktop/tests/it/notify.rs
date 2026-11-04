use crate::dbus_support;
use crate::desktop_support;

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

const LOG: &str = "ES_FAKE_NOTIFY_LOG";

struct Activatable {
    log: std::path::PathBuf,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Activatable {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app_name: String,
        _replaces_id: u32,
        _app_icon: String,
        summary: &str,
        _body: String,
        _actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        append(&self.log, &format!("notify {summary}"));
        1
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["actions".into()]
    }
}

fn append(log: &std::path::Path, line: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .expect("log");
    writeln!(f, "{line}").expect("write log");
}

fn lines(log: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

// Run by the bus as the activated service; a plain run of the test binary skips it.
#[test]
fn activatable_fake_server() {
    let Some(log) = std::env::var_os(LOG).map(std::path::PathBuf::from) else {
        return;
    };
    append(&log, "started");
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("bus address");
    let _server = run(serve_on(
        &address,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        Activatable { log },
    ));
    std::thread::sleep(Duration::from_secs(60));
}

fn bus_with_installed_server(dir: &tempfile::TempDir) -> (QuietBus, std::path::PathBuf) {
    let services = dir.path().join("services");
    std::fs::create_dir(&services).expect("services dir");
    let log = dir.path().join("log");
    let exe = std::env::current_exe().expect("test binary");
    let script = dir.path().join("fake.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\n{LOG}={} exec {} --exact notify::activatable_fake_server >/dev/null 2>&1\n",
            log.display(),
            exe.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("chmod");
    std::fs::write(
        services.join("fake.service"),
        format!(
            "[D-BUS Service]\nName=org.freedesktop.Notifications\nExec={}\n",
            script.display()
        ),
    )
    .expect("service file");
    (QuietBus::start_with_services(&services), log)
}

#[test]
fn starting_the_notifier_does_not_activate_an_installed_server() {
    let dir = tempfile::tempdir().expect("tmp");
    let (bus, log) = bus_with_installed_server(&dir);
    let _notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(lines(&log).is_empty(), "the installed server was started");
}

#[test]
fn a_server_that_claims_the_name_later_gets_the_first_notification() {
    let dir = tempfile::tempdir().expect("tmp");
    let (bus, log) = bus_with_installed_server(&dir);
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");
    assert!(!notifier.supports_actions());

    let (_server, received) = run(fake(&bus, &["actions"]));
    std::thread::sleep(Duration::from_millis(500));
    assert!(notifier.supports_actions());
    notifier.notify(notification()).expect("notify");
    assert_eq!(run(received.recv()).expect("received").summary, "Saved");
    assert!(lines(&log).is_empty(), "the installed server was started");
}

#[test]
fn an_action_from_a_server_that_claimed_the_name_after_start_reaches_the_channel() {
    let bus = QuietBus::start();
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");
    let (server, received) = run(fake(&bus, &["actions"]));
    let id = notifier.notify(notification()).expect("notify");
    run(received.recv()).expect("received");
    run(emit(&server, id.0, "show"));
    let got = run(notifier.actions().recv()).expect("action");
    assert_eq!(got, (id, "show".to_owned()));
}

#[test]
fn with_no_server_running_the_first_notification_activates_the_installed_one() {
    let dir = tempfile::tempdir().expect("tmp");
    let (bus, log) = bus_with_installed_server(&dir);
    let notifier = run(DesktopNotifier::connect(&bus.endpoint())).expect("connect");

    notifier.notify(notification()).expect("notify");
    assert_eq!(lines(&log), ["started", "notify Saved"]);
}
