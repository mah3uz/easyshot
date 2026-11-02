use es_desktop::{
    Notification, NotificationId, Notifier, RecordingNotifier, RecordingShortcuts, RecordingSound,
    RecordingTray, Shortcuts, Sound, Tray, TrayModel, Urgency,
};
use es_protocol::{CATALOGUE, Route};

fn note() -> Notification {
    Notification {
        title: "t".into(),
        body: "b".into(),
        icon: None,
        image: None,
        actions: vec![("open".into(), "Open".into())],
        urgency: Urgency::Normal,
        timeout: None,
    }
}

#[test]
fn the_recording_notifier_keeps_what_was_sent_and_replays_invoked_actions() {
    let notifier = RecordingNotifier::default();
    let first = notifier.notify(note()).expect("notify");
    let second = notifier.notify(note()).expect("notify");
    assert_ne!(first, second);
    assert_eq!(notifier.sent().len(), 2);
    notifier.invoke(second, "open");
    assert_eq!(
        notifier.actions().try_recv().ok(),
        Some((NotificationId(second.0), "open".to_owned()))
    );
    assert!(!RecordingNotifier::new(false).supports_actions());
}

#[test]
fn the_recording_tray_and_shortcuts_keep_calls_and_forward_activations() {
    let tray = RecordingTray::default();
    tray.set_menu(TrayModel::default());
    tray.set_recording(None);
    assert_eq!(tray.menus().len(), 1);
    assert_eq!(tray.recording_states(), [None]);
    tray.activate(Route::Quit);
    assert_eq!(tray.activations().try_recv().ok(), Some(Route::Quit));

    let shortcuts = RecordingShortcuts::default();
    assert!(shortcuts.user_binds_required());
    shortcuts.register(CATALOGUE).expect("register");
    assert_eq!(shortcuts.registered()[0].len(), CATALOGUE.len());
    shortcuts.activate(Route::Gallery);
    assert_eq!(
        shortcuts.activations().try_recv().ok(),
        Some(Route::Gallery)
    );
}

#[test]
fn the_recording_sound_counts_shutters() {
    let sound = RecordingSound::default();
    sound.shutter();
    sound.shutter();
    assert_eq!(sound.shutters(), 2);
}
