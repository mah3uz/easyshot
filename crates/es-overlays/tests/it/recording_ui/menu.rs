use super::*;

#[test]
fn a_long_window_menu_stays_within_the_output_and_scrolls_to_its_last_row() {
    let mut s = Surface::open(WIDTH as f32, 480., |_| {
        RecordingOptions::new(vec![output("A", 0., 1280., 720.)], devices())
    });
    let (events, _sub) = s.record::<OptionsEvent>();
    let view = s.view.clone();
    let many: Vec<WindowInfo> = (0..60)
        .map(|i| placed(&format!("w{i}"), Some("1"), true))
        .collect();
    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.set_windows(many, cx)));
    s.click(WINDOW_X, ROW_Y);
    let height = s.app.cx.update(|cx| view.read(cx).size().height);
    assert!(height <= 720. * 0.6, "the panel grew to {height}");

    let scrolled = s
        .app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.menu_key("end", cx)));
    assert!(scrolled);
    let offset = s.app.cx.update(|cx| view.read(cx).menu_offset());
    assert!(offset > 0., "End did not scroll the menu");
    s.click(LESS_X, 350.);
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::RefreshWindows)
    ));

    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.menu_key("home", cx)));
    let top = s.app.cx.update(|cx| view.read(cx).menu_offset());
    assert_eq!(top, 0.);
}

fn app_window(id: &str, app: &str, title: &str, workspace: &str, shown: bool) -> WindowInfo {
    WindowInfo {
        app_id: app.into(),
        title: title.into(),
        ..placed(id, Some(workspace), shown)
    }
}

fn desk() -> Vec<WindowInfo> {
    vec![
        app_window(
            "code",
            "com.example.vscode-sdjfhsk",
            "options.rs - easyshot",
            "1",
            true,
        ),
        app_window(
            "fox",
            "org.mozilla.firefox",
            "Rust 1.98 release notes — Mozilla Firefox",
            "1",
            true,
        ),
        app_window("term", "com.example.Terminal", "", "1", true),
        app_window("notes", "com.example.Notes", "Notes", "2", false),
        app_window("files", "org.gnome.Nautilus", "Downloads", "2", false),
        app_window(
            "chat",
            "com.example.Slack-x8f2k1",
            "general | A team with a rather long name that will not fit on one line",
            "2",
            false,
        ),
        app_window(
            "mail",
            "org.gnome.Evolution",
            "Inbox (12 unread)",
            "mail",
            false,
        ),
    ]
}

fn open_desk(height: f32) -> (Surface<RecordingOptions>, gpui::Entity<RecordingOptions>) {
    let mut s = Surface::open(WIDTH as f32, height, |_| {
        RecordingOptions::new(vec![output("A", 0., 1920., 1080.)], devices())
    });
    let view = s.view.clone();
    s.app
        .cx
        .update(|cx| view.update(cx, |o, cx| o.set_windows(desk(), cx)));
    s.click(WINDOW_X, ROW_Y);
    (s, view)
}

#[test]
fn the_window_menu_names_each_window_by_title_and_app_never_by_its_id() {
    let (mut s, view) = open_desk(640.);
    s.app.screenshot("recording-window-menu");
    let view2 = view.clone();
    s.app.cx.update(|cx| {
        es_ui::theme::set_mode(ThemeMode::Light, cx);
        view2.update(cx, |_, cx| cx.notify());
    });
    s.app.screenshot("recording-window-menu-light");

    let labels = s.app.cx.update(|cx| view.read(cx).menu_labels());
    for raw in ["com.", "org.", "sdjfhsk", "x8f2k1"] {
        assert!(
            labels.iter().all(|l| !l.contains(raw)),
            "an app id reached the menu: {labels:?}"
        );
    }
    let has = |l: &str| labels.iter().any(|x| x == l);
    assert!(has("options.rs - easyshot — Vscode"), "{labels:?}");
    assert!(has("Terminal"), "an untitled window reads as its app");
    assert!(has("Notes"), "a title that is the app's name shows once");
    assert!(!labels.iter().any(|l| l.starts_with("Notes —")));
    assert_eq!(labels[0], "Workspace 1 · on screen");
    assert!(has("Workspace 2") && has("Workspace mail"));
    assert_eq!(labels.last().map(String::as_str), Some("Refresh Windows"));
}

#[test]
fn the_keyboard_and_the_pointer_share_one_highlighted_window() {
    let (mut s, view) = open_desk(640.);
    let (events, _sub) = s.record::<OptionsEvent>();
    let key = |s: &mut Surface<RecordingOptions>, k: &'static str| {
        s.app
            .cx
            .update(|cx| view.update(cx, |o, cx| o.menu_key(k, cx)))
    };
    assert!(
        !key(&mut s, "enter"),
        "Enter with nothing chosen is left to the bar"
    );
    assert!(key(&mut s, "down") && key(&mut s, "down"));
    s.app.screenshot("recording-window-menu-keyboard");
    assert!(key(&mut s, "enter"));
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::Start(r)) if matches!(&r.target, RecordingTarget::Window(w) if w.id.0 == "fox")
    ));

    s.click(WINDOW_X, ROW_Y);
    // The workspace label, then the first window.
    s.hover(LESS_X, MENU_TOP + 28. + 16., false);
    key(&mut s, "down");
    key(&mut s, "down");
    key(&mut s, "enter");
    assert!(
        matches!(
            events.borrow().last(),
            Some(OptionsEvent::Start(r)) if matches!(&r.target, RecordingTarget::Window(w) if w.id.0 == "term")
        ),
        "Down goes on from the window under the pointer"
    );

    s.click(WINDOW_X, ROW_Y);
    key(&mut s, "end");
    key(&mut s, "enter");
    assert!(matches!(
        events.borrow().last(),
        Some(OptionsEvent::RefreshWindows)
    ));
}
