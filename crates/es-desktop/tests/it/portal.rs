use crate::dbus_support;
use crate::desktop_support;

use dbus_support::run;
use desktop_support::{QuietBus, serve_on};
use es_desktop::{
    Appearance, AppearanceEvent, ColorScheme, Error, FilePicker, PortalAppearance,
    PortalFilePicker, PortalShortcuts, SaveRequest, Shortcuts,
};
use es_protocol::CATALOGUE;
use std::sync::{Arc, Mutex};
use zbus::zvariant::{OwnedValue, Value};

const PATH: &str = "/org/freedesktop/portal/desktop";

struct Settings {
    values: Arc<Mutex<(u32, bool)>>,
}

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }

    fn read(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
        let (scheme, animations) = *self
            .values
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("poisoned".into()))?;
        match (namespace, key) {
            ("org.freedesktop.appearance", "color-scheme") => Ok(OwnedValue::from(scheme)),
            ("org.gnome.desktop.interface", "enable-animations") => {
                Ok(OwnedValue::from(animations))
            }
            _ => Err(zbus::fdo::Error::Failed("unknown setting".into())),
        }
    }

    #[zbus(signal)]
    async fn setting_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: Value<'_>,
    ) -> zbus::Result<()>;
}

async fn announce(conn: &zbus::Connection, namespace: &str, key: &str, value: Value<'_>) {
    let emitter = zbus::object_server::SignalEmitter::new(conn, PATH).expect("emitter");
    Settings::setting_changed(&emitter, namespace, key, value)
        .await
        .expect("signal");
}

fn labels() -> es_desktop::Labels {
    Arc::new(|id| id.to_owned())
}

#[test]
fn colour_scheme_and_reduce_motion_follow_the_settings_portal() {
    let bus = QuietBus::start();
    let values = Arc::new(Mutex::new((1u32, true)));
    let portal = run(serve_on(
        &bus.address,
        "org.freedesktop.portal.Desktop",
        PATH,
        Settings {
            values: values.clone(),
        },
    ));
    let appearance = run(PortalAppearance::start(&bus.endpoint())).expect("start");
    assert_eq!(appearance.color_scheme(), ColorScheme::Dark);
    assert!(!appearance.reduce_motion());

    let changes = appearance.changes();
    run(announce(
        &portal,
        "org.freedesktop.appearance",
        "color-scheme",
        Value::U32(2),
    ));
    assert_eq!(
        run(changes.recv()).expect("event"),
        AppearanceEvent::ColorScheme(ColorScheme::Light)
    );
    assert_eq!(appearance.color_scheme(), ColorScheme::Light);

    run(announce(
        &portal,
        "org.gnome.desktop.interface",
        "enable-animations",
        Value::Bool(false),
    ));
    assert_eq!(
        run(changes.recv()).expect("event"),
        AppearanceEvent::ReduceMotion(true)
    );
    assert!(appearance.reduce_motion());
}

#[test]
fn animations_off_at_startup_means_reduce_motion() {
    let bus = QuietBus::start();
    let _portal = run(serve_on(
        &bus.address,
        "org.freedesktop.portal.Desktop",
        PATH,
        Settings {
            values: Arc::new(Mutex::new((2, false))),
        },
    ));
    let appearance = run(PortalAppearance::start(&bus.endpoint())).expect("start");
    assert_eq!(appearance.color_scheme(), ColorScheme::Light);
    assert!(appearance.reduce_motion());
}

#[test]
fn without_a_portal_appearance_has_defaults_and_shortcuts_need_user_binds() {
    let bus = QuietBus::start();
    let appearance = run(PortalAppearance::start(&bus.endpoint())).expect("start");
    assert_eq!(appearance.color_scheme(), ColorScheme::Dark);
    assert!(!appearance.reduce_motion());

    let shortcuts = run(PortalShortcuts::start(&bus.endpoint(), labels()));
    assert!(shortcuts.user_binds_required());
    let result = shortcuts.register(CATALOGUE);
    assert!(matches!(result, Err(Error::Unsupported)), "{result:?}");
}

#[test]
fn a_missing_file_chooser_portal_is_an_error_not_a_hang() {
    let bus = QuietBus::start();
    let picker = PortalFilePicker::new(bus.endpoint());
    let saved = run(picker.save(SaveRequest {
        title: "Save".into(),
        suggested_name: "a.png".into(),
        dir: None,
    }));
    assert!(saved.is_err());
}
