#![allow(dead_code)]

use es_desktop::dbus::{Callbacks, Endpoint, Service};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

pub struct PrivateBus {
    child: Child,
    pub address: String,
}

impl PrivateBus {
    pub fn start() -> PrivateBus {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is installed");
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("piped"))
            .read_line(&mut address)
            .expect("bus address");
        PrivateBus {
            child,
            address: address.trim().to_owned(),
        }
    }

    pub fn endpoint(&self) -> Endpoint {
        Endpoint::session().on_address(&self.address)
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn callbacks() -> Callbacks {
    Callbacks {
        doctor: Arc::new(|| Box::pin(async { r#"{"ok":true}"#.to_owned() })),
        #[cfg(feature = "test-hooks")]
        dump: Arc::new(|| Box::pin(async { "state".to_owned() })),
    }
}

pub async fn service(endpoint: &Endpoint) -> Service {
    Service::start(endpoint.clone(), callbacks())
        .await
        .expect("service starts")
}

pub fn run<F: std::future::Future>(f: F) -> F::Output {
    use futures::FutureExt;
    let (tx, rx) = async_channel::bounded::<()>(1);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        let _ = tx.send_blocking(());
    });
    futures::executor::block_on(async {
        futures::select! {
            out = f.fuse() => out,
            _ = rx.recv().fuse() => panic!("timed out waiting on the bus"),
        }
    })
}
