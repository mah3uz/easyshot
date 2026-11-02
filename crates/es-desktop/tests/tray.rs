mod dbus_support;
mod desktop_support;

use dbus_support::run;
use desktop_support::{on_private_session, serve_on};
use es_desktop::dbus::Endpoint;
use es_desktop::{KsniTray, RecordingTrayState, Tray, TrayItem, TrayModel, watcher_present};
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
        "the_tray_registers_shows_the_menu_and_routes_clicks",
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
                (t != "Easyshot").then_some(t)
            }));
            assert_eq!(title, "Easyshot 01:15");
            let recording: Vec<(i32, i32, Vec<u8>)> =
                run(sni.get_property("IconPixmap")).expect("pixmap");
            assert_ne!(recording, idle, "the recording dot is drawn on the pixmap");
        },
    );
}

#[test]
fn without_a_watcher_the_probe_says_so_and_the_tray_does_not_start() {
    on_private_session(
        "without_a_watcher_the_probe_says_so_and_the_tray_does_not_start",
        |address| {
            let endpoint = Endpoint::session().on_address(address);
            assert!(!run(watcher_present(&endpoint)));
            let started = run(KsniTray::start(Arc::new(|id| id.to_owned()), Vec::new()));
            assert!(matches!(started, Err(es_desktop::Error::Unsupported)));
        },
    );
}
