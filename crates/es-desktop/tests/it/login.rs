use crate::desktop_support::{QuietBus, serve_on};
use es_desktop::follow_login_session;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const NAME: &str = "org.freedesktop.login1";
const REAL: &str = "/org/freedesktop/login1/session/_32";

struct Session {
    active: bool,
    locked: bool,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    #[zbus(property)]
    fn id(&self) -> String {
        "2".into()
    }
    #[zbus(property)]
    fn active(&self) -> bool {
        self.active
    }
    #[zbus(property)]
    fn locked_hint(&self) -> bool {
        self.locked
    }
}

struct Manager;

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn get_session(&self, id: &str) -> zbus::fdo::Result<zbus::zvariant::OwnedObjectPath> {
        assert_eq!(id, "2");
        Ok(zbus::zvariant::ObjectPath::try_from(REAL)
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            .into())
    }
}

fn wait_for(open: &AtomicBool, want: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while open.load(Ordering::SeqCst) != want {
        assert!(Instant::now() < deadline, "the gate never became {want}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

// Waits happen outside block_on: a thread blocked inside it stops async-io's reactor for every
// connection.
#[test]
fn the_gate_is_open_only_while_the_session_is_active_and_unlocked() {
    let bus = QuietBus::start();
    let server = zbus::block_on(async {
        let server = serve_on(&bus.address, NAME, "/org/freedesktop/login1", Manager).await;
        for path in ["/org/freedesktop/login1/session/auto", REAL] {
            let s = Session {
                active: true,
                locked: false,
            };
            server.object_server().at(path, s).await.expect("serve");
        }
        server
    });
    let client = zbus::block_on(async {
        zbus::connection::Builder::address(bus.address.as_str())
            .expect("address")
            .build()
            .await
            .expect("client")
    });
    let open = Arc::new(AtomicBool::new(true));
    let o = open.clone();
    let follower = std::thread::spawn(move || {
        zbus::block_on(follow_login_session(&client, &o)).expect("follow");
    });
    let set = |active: bool, locked: bool| {
        zbus::block_on(async {
            let iface = server
                .object_server()
                .interface::<_, Session>(REAL)
                .await
                .expect("session iface");
            let mut s = iface.get_mut().await;
            s.active = active;
            s.locked = locked;
            s.active_changed(iface.signal_emitter())
                .await
                .expect("emit");
            s.locked_hint_changed(iface.signal_emitter())
                .await
                .expect("emit");
        });
    };
    std::thread::sleep(Duration::from_millis(200));
    wait_for(&open, true);
    set(true, true);
    wait_for(&open, false);
    set(true, false);
    wait_for(&open, true);
    set(false, false);
    wait_for(&open, false);
    set(true, false);
    wait_for(&open, true);
    drop(server);
    wait_for(&open, false);
    follower.join().expect("the follower ends with the bus");
}
