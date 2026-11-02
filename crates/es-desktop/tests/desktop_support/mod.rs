#![allow(dead_code)]

use es_desktop::dbus::Endpoint;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

const CHILD: &str = "ES_TEST_CHILD";

// Process-wide environment cannot be changed in-process, so the test re-runs itself with the variables set.
pub fn isolated(test: &str, envs: &[(&str, &str)], body: impl FnOnce()) {
    if std::env::var_os(CHILD).is_some() {
        body();
        return;
    }
    let exe = std::env::current_exe().expect("test binary path");
    let status = Command::new(exe)
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .envs(envs.iter().copied())
        .env(CHILD, "1")
        .status()
        .expect("re-run the test");
    assert!(status.success(), "the isolated run of {test} failed");
}

pub fn on_private_session(test: &str, body: impl FnOnce(&str)) {
    if std::env::var_os(CHILD).is_some() {
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("session bus address");
        body(&address);
        return;
    }
    let bus = QuietBus::start();
    isolated(test, &[("DBUS_SESSION_BUS_ADDRESS", &bus.address)], || {});
}

pub async fn serve_on(
    address: &str,
    name: &str,
    path: &str,
    iface: impl zbus::object_server::Interface,
) -> zbus::Connection {
    zbus::connection::Builder::address(address)
        .expect("address")
        .name(name.to_owned())
        .expect("name")
        .serve_at(path.to_owned(), iface)
        .expect("serve")
        .build()
        .await
        .expect("server")
}

// A bus with no service directories, so nothing on the machine can be activated on it.
pub struct QuietBus {
    child: Child,
    pub address: String,
}

impl QuietBus {
    pub fn start() -> QuietBus {
        let config = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("quiet-bus-{}.conf", std::process::id()));
        std::fs::write(
            &config,
            r#"<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>"#,
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
        QuietBus {
            child,
            address: address.trim().to_owned(),
        }
    }

    pub fn endpoint(&self) -> Endpoint {
        Endpoint::session().on_address(&self.address)
    }
}

impl Drop for QuietBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
