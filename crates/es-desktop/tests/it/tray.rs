use crate::dbus_support;
use crate::desktop_support;

use dbus_support::run;
use desktop_support::{on_private_session, serve_on};
use es_desktop::dbus::Endpoint;
use es_desktop::{
    KsniTray, RecordingTrayState, Tray, TrayItem, TrayModel, host_within, watcher_present,
};
use es_protocol::Route;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use zbus::zvariant::{OwnedValue, Value};

struct Watcher {
    tx: async_channel::Sender<String>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    async fn register_status_notifier_item(&self, service: String) {
        let _ = self.tx.send(service).await;
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

fn item(label_id: &'static str, route: Option<Route>) -> TrayItem {
    TrayItem {
        label_id,
        label: None,
        shortcut: None,
        route,
        children: Vec::new(),
        enabled: true,
        separator: false,
    }
}

fn labels(children: &[OwnedValue]) -> Vec<(i32, String)> {
    children
        .iter()
        .filter_map(|c| {
            let Value::Structure(node) = &**c else {
                return None;
            };
            let fields = node.fields();
            let id = i32::try_from(fields.first()?).ok()?;
            let Value::Dict(props) = fields.get(1)? else {
                return None;
            };
            let label: String = props.get::<&str, &str>(&"label").ok()??.to_owned();
            Some((id, label))
        })
        .collect()
}

async fn layout(conn: &zbus::Connection, service: &str) -> Vec<(i32, String)> {
    let menu = zbus::Proxy::new(
        conn,
        service.to_owned(),
        "/MenuBar",
        "com.canonical.dbusmenu",
    )
    .await
    .expect("menu proxy");
    let (_, (_, _, children)): (u32, Layout) = menu
        .call("GetLayout", &(0i32, -1i32, Vec::<String>::new()))
        .await
        .expect("layout");
    labels(&children)
}

async fn wait_for<T>(mut probe: impl AsyncFnMut() -> Option<T>) -> T {
    for _ in 0..100 {
        if let Some(v) = probe().await {
            return v;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("the tray never reached the expected state");
}

#[test]
fn the_tray_registers_shows_the_menu_and_routes_clicks() {
    on_private_session(
        "tray::the_tray_registers_shows_the_menu_and_routes_clicks",
        |address| {
            let endpoint = Endpoint::session().on_address(address);
            let (tx, registered) = async_channel::unbounded();
            let _watcher = run(serve_on(
                address,
                "org.kde.StatusNotifierWatcher",
                "/StatusNotifierWatcher",
                Watcher { tx },
            ));
            assert!(run(watcher_present(&endpoint)));

            let tray = run(KsniTray::start(
                Arc::new(|id| format!("label:{id}")),
                es_desktop::tray_icons(),
            ))
            .expect("tray starts");
            let service = run(registered.recv()).expect("registered");
            let activations = tray.activations();

            tray.set_menu(TrayModel {
                items: vec![
                    item("tray-region", Some(Route::CaptureRegion)),
                    item("tray-gallery", Some(Route::Gallery)),
                ],
            });
            let client = run(zbus::Connection::session()).expect("client");
            let shown = run(wait_for(async || {
                Some(layout(&client, &service).await).filter(|l| !l.is_empty())
            }));
            let texts: Vec<&str> = shown.iter().map(|(_, l)| l.as_str()).collect();
            assert_eq!(texts, ["label:tray-region", "label:tray-gallery"]);

            let menu = run(zbus::Proxy::new(
                &client,
                service.clone(),
                "/MenuBar",
                "com.canonical.dbusmenu",
            ))
            .expect("menu proxy");
            let gallery = shown[1].0;
            run(menu.call_method(
                "Event",
                &(gallery, "clicked", zbus::zvariant::Value::I32(0), 0u32),
            ))
            .expect("click");
            assert_eq!(run(activations.recv()).expect("route"), Route::Gallery);

            let sni = run(zbus::proxy::Builder::<zbus::Proxy>::new(&client)
                .destination(service.clone())
                .expect("destination")
                .path("/StatusNotifierItem")
                .expect("path")
                .interface("org.kde.StatusNotifierItem")
                .expect("interface")
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build())
            .expect("item proxy");
            run(sni.call_method("Activate", &(0i32, 0i32))).expect("activate");
            assert_eq!(run(activations.recv()).expect("route"), Route::CaptureBar);

            let icon: String = run(sni.get_property("IconName")).expect("icon");
            assert_eq!(icon, "", "an empty name makes every host use the pixmap");
            let idle: Vec<(i32, i32, Vec<u8>)> =
                run(sni.get_property("IconPixmap")).expect("pixmap");
            assert_eq!(
                idle.iter().map(|p| p.0).collect::<Vec<_>>(),
                [16, 22, 32, 44]
            );
            assert!(
                idle.iter()
                    .all(|(w, h, d)| d.len() == (w * h * 4) as usize
                        && d.chunks(4).any(|p| p[0] > 0))
            );

            tray.set_recording(Some(RecordingTrayState {
                elapsed: Duration::from_secs(75),
                paused: false,
            }));
            let title = run(wait_for(async || {
                let t: String = sni.get_property("Title").await.ok()?;
                (t != "label:tray-title").then_some(t)
            }));
            assert_eq!(title, "label:tray-title 01:15");
            let recording: Vec<(i32, i32, Vec<u8>)> =
                run(sni.get_property("IconPixmap")).expect("pixmap");
            assert_ne!(recording, idle, "the recording dot is drawn on the pixmap");
        },
    );
}

fn after(delay: Duration) -> impl std::future::Future<Output = ()> {
    let (tx, rx) = futures::channel::oneshot::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _ = tx.send(());
    });
    async move {
        let _ = rx.await;
    }
}

fn serve_watcher_after(
    address: &str,
    delay: Duration,
) -> (
    async_channel::Receiver<String>,
    std::thread::JoinHandle<zbus::Connection>,
) {
    let (tx, registered) = async_channel::unbounded();
    let address = address.to_owned();
    let served = std::thread::spawn(move || {
        std::thread::sleep(delay);
        run(serve_on(
            &address,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            Watcher { tx },
        ))
    });
    (registered, served)
}

#[test]
fn a_watcher_that_appears_after_the_tray_started_gets_the_registration() {
    on_private_session(
        "tray::a_watcher_that_appears_after_the_tray_started_gets_the_registration",
        |address| {
            let endpoint = Endpoint::session().on_address(address);
            assert!(!run(watcher_present(&endpoint)));
            let tray = run(KsniTray::start(Arc::new(|id| id.to_owned()), Vec::new()));
            assert!(tray.is_ok(), "the tray starts without a watcher");

            let (registered, served) = serve_watcher_after(address, Duration::from_millis(400));
            assert!(run(host_within(&endpoint, after(Duration::from_secs(5)))));
            let service = run(registered.recv()).expect("registered once the watcher is up");
            assert!(service.starts_with("org.kde.StatusNotifierItem-"));
            let _watcher = served.join().expect("watcher thread");
        },
    );
}

#[test]
fn a_watcher_that_never_appears_is_reported_after_the_deadline() {
    on_private_session(
        "tray::a_watcher_that_never_appears_is_reported_after_the_deadline",
        |address| {
            let endpoint = Endpoint::session().on_address(address);
            let started = std::time::Instant::now();
            let found = run(host_within(&endpoint, after(Duration::from_millis(600))));
            assert!(!found);
            assert!(started.elapsed() >= Duration::from_millis(600));
        },
    );
}

#[test]
fn a_restarted_watcher_gets_the_registration_again() {
    on_private_session(
        "tray::a_restarted_watcher_gets_the_registration_again",
        |address| {
            let (tx, registered) = async_channel::unbounded();
            let first = run(serve_on(
                address,
                "org.kde.StatusNotifierWatcher",
                "/StatusNotifierWatcher",
                Watcher { tx },
            ));
            let _tray = run(KsniTray::start(Arc::new(|id| id.to_owned()), Vec::new()))
                .expect("tray starts");
            let service = run(registered.recv()).expect("first registration");

            run(first.close()).expect("watcher goes away");
            let (registered, served) = serve_watcher_after(address, Duration::from_millis(300));
            let again = run(registered.recv()).expect("registered with the new watcher");
            assert_eq!(again, service);
            let _second = served.join().expect("watcher thread");
        },
    );
}
