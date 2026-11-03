use crate::desktop_support::{QuietBus, serve_on};
use es_desktop::follow_login_session;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use zbus::object_server::SignalEmitter;

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
    #[zbus(signal)]
    async fn lock(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn unlock(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
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

fn stays(open: &AtomicBool, want: bool) {
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(open.load(Ordering::SeqCst), want);
}

struct Rig {
    _bus: QuietBus,
    server: zbus::Connection,
    open: Arc<AtomicBool>,
    follower: JoinHandle<()>,
}

// Waits happen outside block_on: a thread blocked inside it stops async-io's reactor for every
// connection.
impl Rig {
    fn start(screen: Option<async_channel::Receiver<bool>>) -> Rig {
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
            let mut requested = false;
            zbus::block_on(follow_login_session(&client, &o, screen, &mut requested))
                .expect("follow");
        });
        std::thread::sleep(Duration::from_millis(200));
        Rig {
            _bus: bus,
            server,
            open,
            follower,
        }
    }

    fn set(&self, active: bool, locked: bool) {
        zbus::block_on(async {
            let iface = self
                .server
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
    }

    fn signal(&self, lock: bool) {
        zbus::block_on(async {
            let iface = self
                .server
                .object_server()
                .interface::<_, Session>(REAL)
                .await
                .expect("session iface");
            let emitter = iface.signal_emitter();
            if lock {
                Session::lock(emitter).await.expect("lock");
            } else {
                Session::unlock(emitter).await.expect("unlock");
            }
        });
    }

    fn end(self) {
        drop(self.server);
        wait_for(&self.open, false);
        self.follower
            .join()
            .expect("the follower ends with the bus");
    }
}

#[test]
fn the_gate_is_open_only_while_the_session_is_active_and_unlocked() {
    let rig = Rig::start(None);
    wait_for(&rig.open, true);
    rig.set(true, true);
    wait_for(&rig.open, false);
    rig.set(true, false);
    wait_for(&rig.open, true);
    rig.set(false, false);
    wait_for(&rig.open, false);
    rig.set(true, false);
    wait_for(&rig.open, true);
    rig.end();
}

// Lockers on wlroots compositors never set LockedHint; a session asked to lock is taken as locked
// until it is asked to unlock.
#[test]
fn a_lock_signal_closes_the_gate_until_an_unlock_signal() {
    let rig = Rig::start(None);
    wait_for(&rig.open, true);
    rig.signal(true);
    wait_for(&rig.open, false);
    rig.set(true, false);
    stays(&rig.open, false);
    rig.signal(false);
    wait_for(&rig.open, true);
    rig.end();
}

#[test]
fn the_compositors_lock_report_closes_the_gate_and_its_unlock_answers_a_lock_signal() {
    let (tx, rx) = async_channel::unbounded();
    tx.try_send(true).expect("send");
    let rig = Rig::start(Some(rx));
    stays(&rig.open, false);
    tx.try_send(false).expect("send");
    wait_for(&rig.open, true);
    tx.try_send(true).expect("send");
    wait_for(&rig.open, false);
    rig.set(true, false);
    stays(&rig.open, false);
    tx.try_send(false).expect("send");
    wait_for(&rig.open, true);
    rig.signal(true);
    wait_for(&rig.open, false);
    tx.try_send(false).expect("send");
    wait_for(&rig.open, true);
    drop(tx);
    wait_for(&rig.open, false);
    rig.end();
}
