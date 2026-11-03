use es_backend::desktop::{
    FREEDESKTOP_NOTIFICATIONS, PORTAL_FILE_CHOOSER, PORTAL_GLOBAL_SHORTCUTS, PORTAL_SETTINGS,
    STATUS_NOTIFIER,
};
use es_backend::{DbusNames, PortalIfaces, Probe};

fn desktop_probe() -> Probe {
    Probe {
        dbus: DbusNames {
            names: [
                "org.kde.StatusNotifierWatcher",
                "org.freedesktop.Notifications",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
        },
        portals: PortalIfaces {
            global_shortcuts: true,
            file_chooser: true,
            settings: true,
            ..PortalIfaces::default()
        },
        ..Probe::default()
    }
}

#[test]
fn desktop_providers_probe_their_bus_names_and_portals() {
    let p = desktop_probe();
    assert_eq!((STATUS_NOTIFIER.probe)(&p), Some(10));
    assert_eq!((FREEDESKTOP_NOTIFICATIONS.probe)(&p), Some(10));
    assert_eq!((PORTAL_GLOBAL_SHORTCUTS.probe)(&p), Some(10));
    assert_eq!((PORTAL_FILE_CHOOSER.probe)(&p), Some(10));
    assert_eq!((PORTAL_SETTINGS.probe)(&p), Some(10));
    let bare = Probe::default();
    assert_eq!((STATUS_NOTIFIER.probe)(&bare), None);
    assert_eq!((FREEDESKTOP_NOTIFICATIONS.probe)(&bare), None);
    assert_eq!((PORTAL_GLOBAL_SHORTCUTS.probe)(&bare), None);
    assert_eq!((PORTAL_FILE_CHOOSER.probe)(&bare), None);
    assert_eq!((PORTAL_SETTINGS.probe)(&bare), None);
}

#[cfg(feature = "test-hooks")]
#[test]
fn test_mode_builds_doubles_and_never_opens_portals() {
    use es_backend::TestConfig;
    use es_desktop::TrayModel;
    use es_model::platform::OutputId;

    let p = Probe {
        test: Some(TestConfig {
            output: OutputId("TEST-1".into()),
            dir: std::env::temp_dir(),
            virtual_input: false,
        }),
        ..desktop_probe()
    };
    // Invisible doubles are checked first, so a regression fails before a real tray icon could appear.
    let notifier = (FREEDESKTOP_NOTIFICATIONS.build)(&p).expect("recording notifier");
    assert!(!notifier.supports_actions());
    let shortcuts = (PORTAL_GLOBAL_SHORTCUTS.build)(&p).expect("recording shortcuts");
    assert!(!shortcuts.user_binds_required());
    for err in [
        (PORTAL_FILE_CHOOSER.build)(&p).err(),
        (PORTAL_SETTINGS.build)(&p).err(),
    ] {
        let err = err.expect("no portal in test mode");
        assert!(err.to_string().contains("test mode"), "{err}");
    }
    let tray = (STATUS_NOTIFIER.build)(&p).expect("recording tray");
    tray.set_menu(TrayModel::default());
}
