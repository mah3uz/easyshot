use std::path::PathBuf;
use std::time::Duration;

use es_desktop::{RecordingTray, RecordingTrayState, Tray, TrayItem};
use es_protocol::{Route, Trigger};
use es_shell::tray::{self, Recent, TrayIcon, TrayInput};

fn recent(prefix: &str, n: usize) -> Vec<Recent> {
    (0..n)
        .map(|i| Recent {
            name: format!("{prefix} {i}"),
            file: PathBuf::from(format!("/captures/{prefix}-{i}.png")),
        })
        .collect()
}

fn input() -> TrayInput {
    TrayInput {
        shortcuts: vec![
            (
                Route::CaptureRegion,
                Trigger::parse("Super+Shift+4").expect("trigger"),
            ),
            (
                Route::Ocr,
                Trigger::parse("Super+Shift+O").expect("trigger"),
            ),
        ],
        screenshots: recent("Screenshot", 11),
        recordings: Vec::new(),
        has_pins: false,
        recorder_missing: false,
        recording: None,
        version: "0.1.0".into(),
    }
}

fn shown(tray: &RecordingTray) -> Vec<TrayItem> {
    tray.menus().pop().expect("a menu was set").items
}

fn ids(items: &[TrayItem]) -> Vec<&'static str> {
    items
        .iter()
        .map(|i| if i.separator { "---" } else { i.label_id })
        .collect()
}

fn find<'a>(items: &'a [TrayItem], id: &str) -> &'a TrayItem {
    items
        .iter()
        .find(|i| i.label_id == id)
        .unwrap_or_else(|| panic!("{id} is not in the menu"))
}

const IDLE_ORDER: [&str; 13] = [
    "tray-region",
    "tray-screen",
    "tray-window",
    "tray-record",
    "tray-scrolling",
    "tray-ocr",
    "tray-pick-color",
    "tray-recent",
    "tray-gallery",
    "---",
    "tray-settings",
    "tray-quit",
    "tray-version",
];

#[test]
fn the_menu_keeps_the_capture_first_order_and_hides_unpin_without_pins() {
    let tray = RecordingTray::default();
    tray::show(&tray, &input());
    let items = shown(&tray);
    assert_eq!(ids(&items), IDLE_ORDER);
    assert_eq!(tray.recording_states(), vec![None]);
    assert_eq!(tray::icon(&input()), TrayIcon::Idle);

    let version = items.last().expect("version line");
    assert_eq!(version.label.as_deref(), Some("Version 0.1.0"));
    assert!(
        !version.enabled && version.route.is_none(),
        "the version line is not a command"
    );
}

#[test]
fn unpin_all_appears_after_the_gallery_only_while_something_is_pinned() {
    let tray = RecordingTray::default();
    tray::show(
        &tray,
        &TrayInput {
            has_pins: true,
            ..input()
        },
    );
    let items = shown(&tray);
    let at = ids(&items);
    let gallery = at
        .iter()
        .position(|&i| i == "tray-gallery")
        .expect("gallery");
    assert_eq!(at[gallery + 1], "tray-unpin-all");
    assert_eq!(at[gallery + 2], "---");
    assert_eq!(find(&items, "tray-unpin-all").route, Some(Route::UnpinAll));
}

#[test]
fn labels_read_from_the_catalogue_and_carry_the_bound_shortcut() {
    let tray = RecordingTray::default();
    tray::show(&tray, &input());
    let items = shown(&tray);
    let region = find(&items, "tray-region");
    assert_eq!(region.label.as_deref(), Some("Region"));
    assert_eq!(region.shortcut.as_deref(), Some("Super+Shift+4"));
    assert_eq!(
        find(&items, "tray-ocr").shortcut.as_deref(),
        Some("Super+Shift+O")
    );
    assert_eq!(
        find(&items, "tray-screen").shortcut,
        None,
        "an unbound action shows no shortcut rather than one it does not have"
    );
    assert_eq!(
        find(&items, "tray-pick-color").label.as_deref(),
        Some("Pick Color")
    );
}

#[test]
fn every_item_routes_to_its_command() {
    let tray = RecordingTray::default();
    tray::show(&tray, &input());
    let items = shown(&tray);
    let expected = [
        ("tray-region", Route::CaptureRegion),
        ("tray-screen", Route::CaptureFullscreen),
        ("tray-window", Route::CaptureWindow),
        ("tray-record", Route::Record),
        ("tray-scrolling", Route::CaptureScroll),
        ("tray-ocr", Route::Ocr),
        ("tray-pick-color", Route::ColorPicker),
        ("tray-gallery", Route::Gallery),
        ("tray-settings", Route::Settings(None)),
        ("tray-quit", Route::Quit),
    ];
    for (id, route) in expected {
        let item = find(&items, id);
        assert_eq!(item.route, Some(route.clone()), "{id}");
        assert!(item.enabled, "{id}");
    }
    let clicked = find(&items, "tray-gallery").route.clone().expect("route");
    tray.activate(clicked);
    assert_eq!(tray.activations().try_recv().ok(), Some(Route::Gallery));
}

#[test]
fn recent_captures_list_the_eight_newest_of_each_kind_and_open_them() {
    let tray = RecordingTray::default();
    tray::show(&tray, &input());
    let items = shown(&tray);
    let recent = find(&items, "tray-recent");
    assert_eq!(
        ids(&recent.children),
        ["tray-recent-screenshots", "tray-recent-recordings"]
    );

    let shots = &recent.children[0].children;
    assert_eq!(shots.len(), 8, "eleven screenshots, eight shown");
    assert_eq!(
        shots[0].label.as_deref(),
        Some("Screenshot 0"),
        "newest first"
    );
    assert_eq!(
        shots[7].route,
        Some(Route::Open(PathBuf::from("/captures/Screenshot-7.png")))
    );

    let recordings = &recent.children[1].children;
    assert_eq!(ids(recordings), ["tray-recent-recordings-empty"]);
    assert!(!recordings[0].enabled && recordings[0].route.is_none());
    assert_eq!(recordings[0].label.as_deref(), Some("No recordings yet"));
}

#[test]
fn a_recent_capture_opens_its_export_when_it_has_one() {
    let entry = es_model::HistoryEntry {
        id: Default::default(),
        created_at: time::OffsetDateTime::UNIX_EPOCH,
        kind: es_model::CaptureKind::Region,
        name: "Shot".into(),
        file: "/library/shot.png".into(),
        export_path: Some("/home/me/Pictures/shot.png".into()),
        pixel_size: (10, 10),
        has_annotations: true,
        share_url: None,
    };
    assert_eq!(
        Recent::from(&entry).file,
        PathBuf::from("/home/me/Pictures/shot.png")
    );
}

#[test]
fn while_recording_the_controls_and_clock_come_first_with_a_red_dot() {
    let state = RecordingTrayState {
        elapsed: Duration::from_secs(83),
        paused: false,
    };
    let live = TrayInput {
        recording: Some(state),
        ..input()
    };
    let tray = RecordingTray::default();
    tray::show(&tray, &live);
    let items = shown(&tray);
    assert_eq!(
        ids(&items)[..6],
        [
            "tray-recording",
            "tray-record-stop",
            "tray-record-pause",
            "tray-record-restart",
            "tray-record-discard",
            "---"
        ]
    );
    let record_at = IDLE_ORDER
        .iter()
        .position(|id| *id == "tray-record")
        .unwrap();
    let mut live_order = IDLE_ORDER.to_vec();
    live_order[record_at] = "tray-record-stop";
    assert_eq!(
        &ids(&items)[6..],
        live_order,
        "the normal menu follows, its Record item now stopping, so no second recording can start"
    );
    assert_eq!(items[6 + record_at].route, Some(Route::RecordStop));
    assert_eq!(items[0].label.as_deref(), Some("Recording 01:23"));
    assert!(!items[0].enabled);
    let routes: Vec<_> = items[1..5].iter().map(|i| i.route.clone()).collect();
    assert_eq!(
        routes,
        [
            Some(Route::RecordStop),
            Some(Route::RecordPause),
            Some(Route::RecordRestart),
            Some(Route::RecordDiscard)
        ]
    );
    assert_eq!(tray.recording_states(), vec![Some(state)]);
    assert_eq!(tray::icon(&live), TrayIcon::Recording);
    assert_eq!(
        items[1..5]
            .iter()
            .map(|i| i.label.clone().unwrap_or_else(|| i.label_id.to_owned()))
            .collect::<Vec<_>>(),
        ["Stop Recording", "Pause", "Restart", "Discard"],
        "the controls read from the shell catalogue"
    );

    let paused = TrayInput {
        recording: Some(RecordingTrayState {
            paused: true,
            ..state
        }),
        ..input()
    };
    let items = tray::menu(&paused).items;
    assert_eq!(items[0].label.as_deref(), Some("Paused 01:23"));
    assert_eq!(items[2].label_id, "tray-record-resume");
    assert_eq!(items[2].route, Some(Route::RecordPause));
    assert_eq!(tray::icon(&paused), TrayIcon::Paused);
}

#[test]
fn the_clock_rolls_into_hours() {
    assert_eq!(tray::elapsed_text(Duration::from_secs(59)), "00:59");
    assert_eq!(tray::elapsed_text(Duration::from_secs(3_725)), "1:02:05");
}

#[test]
fn recent_recordings_lists_only_videos_newest_first() {
    let dir = tempfile::tempdir().expect("temp dir");
    let write = |name: &str, age: u64| {
        let file = dir.path().join(name);
        std::fs::write(&file, b"x").expect("write");
        let when = std::time::SystemTime::now() - Duration::from_secs(age);
        std::fs::File::options()
            .write(true)
            .open(&file)
            .and_then(|f| f.set_modified(when))
            .expect("set mtime");
    };
    write("old.mp4", 100);
    write("new.webm", 10);
    write("notes.txt", 1);
    let names: Vec<_> = tray::recent_recordings(dir.path())
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(names, ["new.webm", "old.mp4"]);
    assert!(tray::recent_recordings(&dir.path().join("missing")).is_empty());
}

#[test]
fn saved_global_shortcuts_become_tray_labels_and_unknown_ids_are_skipped() {
    let saved = [
        ("capture-region".to_owned(), "Super+Shift+4".to_owned()),
        ("no-such-entry".to_owned(), "Super+X".to_owned()),
        ("capture-screen".to_owned(), "not a key".to_owned()),
    ]
    .into_iter()
    .collect();
    let shortcuts = tray::shortcuts(&saved);
    assert_eq!(
        shortcuts,
        vec![(
            Route::CaptureRegion,
            Trigger::parse("Super+Shift+4").unwrap()
        )]
    );
}

#[test]
fn a_missing_recorder_disables_the_record_row_with_an_install_hint() {
    let items = tray::menu(&TrayInput {
        recorder_missing: true,
        ..input()
    })
    .items;
    let record = find(&items, "tray-record");
    assert!(!record.enabled);
    assert_eq!(record.route, None);
    assert!(record.label.as_deref().unwrap().contains("wf-recorder"));
    assert!(find(&items, "tray-ocr").enabled);
}
