use es_backend::registry::{APPEARANCE, FILES, NOTIFIER, Registry, SHORTCUTS};
use es_backend::{Backend, Caps, DbusNames, Env, Probe};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const NOTIFICATIONS: &str = "org.freedesktop.Notifications";

struct Bus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    // Only the services written into `services` can be activated on it, never the machine's.
    fn start(services: &[(&str, &str)]) -> Bus {
        let dir = tempfile::tempdir().expect("temp dir");
        let service_dir = dir.path().join("services");
        std::fs::create_dir(&service_dir).expect("service dir");
        for (name, exec) in services {
            std::fs::write(
                service_dir.join(format!("{name}.service")),
                format!("[D-BUS Service]\nName={name}\nExec={exec}\n"),
            )
            .expect("service file");
        }
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<busconfig><type>session</type><listen>unix:tmpdir={}</listen><servicedir>{}</servicedir><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>"#,
                dir.path().display(),
                service_dir.display()
            ),
        )
        .expect("bus config");
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is installed");
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("piped"))
            .read_line(&mut address)
            .expect("bus address");
        Bus {
            child,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            env: Env::from_vars([("DBUS_SESSION_BUS_ADDRESS", self.address.as_str())]),
            ..Probe::default()
        }
    }

    fn serve(&self, name: &str, path: &str, iface: impl zbus::object_server::Interface) -> Served {
        let conn = zbus::blocking::connection::Builder::address(self.address.as_str())
            .expect("address")
            .name(name.to_owned())
            .expect("name")
            .serve_at(path.to_owned(), iface)
            .expect("serve")
            .build()
            .expect("server");
        Served(conn)
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Served(#[allow(dead_code)] zbus::blocking::Connection);

struct FileChooser;

#[zbus::interface(name = "org.freedesktop.portal.FileChooser")]
impl FileChooser {
    #[zbus(property)]
    fn version(&self) -> u32 {
        4
    }
}

struct Notifications;

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "actions".into()]
    }
}

fn portal_registry() -> Registry {
    Registry {
        files: FILES,
        appearance: APPEARANCE,
        shortcuts: SHORTCUTS,
        ..Registry::EMPTY
    }
}

fn eventually(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_portal_that_starts_after_the_backend_brings_its_file_chooser_and_takes_it_when_it_stops() {
    let bus = Bus::start(&[]);
    let b = Backend::detect_with(&bus.probe(), &portal_registry());
    assert!(!b.caps().contains(Caps::FILE_PICKER));
    assert!(b.files().is_none());
    let changes = b.portal_changes();
    b.follow_portal();

    let portal = bus.serve(PORTAL, PORTAL_PATH, FileChooser);
    eventually("the file chooser", || b.caps().contains(Caps::FILE_PICKER));
    assert!(b.files().is_some());
    let report = b.report();
    assert_eq!(report.chosen("files"), Some("portal-file-chooser"));
    assert_eq!(report.portals["FileChooser"], serde_json::json!(true));
    assert!(!b.caps().contains(Caps::APPEARANCE));
    assert!(changes.try_recv().is_ok(), "the change is announced");

    drop(portal);
    eventually("the file chooser to go", || {
        !b.caps().contains(Caps::FILE_PICKER)
    });
    assert!(b.files().is_none());
    assert_eq!(b.report().chosen("files"), None);
    assert_eq!(b.report().portals["FileChooser"], serde_json::json!(false));
}

#[test]
fn following_a_portal_that_is_only_activatable_never_starts_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let marker = dir.path().join("activated");
    let exec = format!("/usr/bin/touch {}", marker.display());
    let bus = Bus::start(&[(PORTAL, exec.as_str())]);
    let mut probe = bus.probe();
    probe.dbus = DbusNames {
        names: [PORTAL.to_owned()].into_iter().collect(),
    };
    let b = Backend::detect_with(&probe, &portal_registry());
    b.follow_portal();
    std::thread::sleep(Duration::from_millis(500));
    assert!(!marker.exists(), "following the portal activated it");
    assert!(!b.caps().contains(Caps::FILE_PICKER));

    activate(&bus.address, PORTAL);
    eventually("the activation control", || marker.exists());
}

// The service never claims its name, so the reply comes only after the bus's activation timeout.
fn activate(address: &str, name: &'static str) {
    let address = address.to_owned();
    std::thread::spawn(move || {
        let conn = zbus::blocking::connection::Builder::address(address.as_str())
            .expect("address")
            .build()
            .expect("client");
        let _ = zbus::blocking::fdo::DBusProxy::new(&conn)
            .expect("bus proxy")
            .start_service_by_name(zbus::names::WellKnownName::try_from(name).expect("name"), 0);
    });
}

fn notifier_probe(bus: &Bus) -> Probe {
    let mut probe = bus.probe();
    probe.dbus = DbusNames {
        names: [NOTIFICATIONS.to_owned()].into_iter().collect(),
    };
    probe
}

fn notifier_registry() -> Registry {
    Registry {
        notifier: NOTIFIER,
        ..Registry::EMPTY
    }
}

#[test]
fn doctor_reports_notification_buttons_when_the_server_already_runs() {
    let bus = Bus::start(&[]);
    let _server = bus.serve(
        NOTIFICATIONS,
        "/org/freedesktop/Notifications",
        Notifications,
    );
    let b = Backend::detect_with(&notifier_probe(&bus), &notifier_registry());
    let v: serde_json::Value = serde_json::from_str(&b.doctor_json()).expect("valid JSON");
    assert!(
        v["caps"]
            .as_array()
            .expect("caps")
            .contains(&serde_json::json!("NOTIFY_ACTIONS")),
        "{v}"
    );
}

#[test]
fn notification_buttons_follow_a_server_that_starts_after_the_backend() {
    let bus = Bus::start(&[]);
    let b = Backend::detect_with(&notifier_probe(&bus), &notifier_registry());
    assert!(!b.caps().contains(Caps::NOTIFY_ACTIONS));
    let server = bus.serve(
        NOTIFICATIONS,
        "/org/freedesktop/Notifications",
        Notifications,
    );
    eventually("notification buttons", || {
        b.caps().contains(Caps::NOTIFY_ACTIONS)
    });
    drop(server);
    eventually("notification buttons to go", || {
        !b.caps().contains(Caps::NOTIFY_ACTIONS)
    });
}

#[test]
fn an_unowned_notification_name_is_never_activated() {
    let dir = tempfile::tempdir().expect("temp dir");
    let marker = dir.path().join("activated");
    let exec = format!("/usr/bin/touch {}", marker.display());
    let bus = Bus::start(&[(NOTIFICATIONS, exec.as_str())]);
    let b = Backend::detect_with(&notifier_probe(&bus), &notifier_registry());
    let _ = b.doctor_json();
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !marker.exists(),
        "the doctor report activated the notification server"
    );
    assert!(!b.caps().contains(Caps::NOTIFY_ACTIONS));

    activate(&bus.address, NOTIFICATIONS);
    eventually("the activation control", || marker.exists());
}
