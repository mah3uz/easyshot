use super::*;

fn wait_until(
    cx: &mut TestAppContext,
    what: &str,
    mut done: impl FnMut(&mut TestAppContext) -> bool,
) {
    cx.executor().allow_parking();
    for _ in 0..100 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn history_limit(rig: &Rig, cx: &mut TestAppContext) -> u32 {
    cx.update(|cx| rig.settings.read(cx).config().general.history_limit)
}

fn settle(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    std::thread::sleep(std::time::Duration::from_millis(600));
    cx.run_until_parked();
}

#[gpui::test]
fn a_hand_edit_applies_live_and_survives_the_next_settings_save(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.paths.config_file();
    std::fs::write(&file, "[general]\nhistory_limit = 7\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 7);

    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.play_sound = false, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("history_limit = 7"), "{text}");
    assert!(text.contains("play_sound = false"), "{text}");
}

#[gpui::test]
fn an_edit_made_while_a_save_is_pending_wins(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 3, cx))
    });
    std::fs::write(rig.paths.config_file(), "[general]\nhistory_limit = 9\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 9);
    settle(cx);
    assert_eq!(history_limit(&rig, cx), 9);
    let text = std::fs::read_to_string(rig.paths.config_file()).unwrap();
    assert!(text.contains("history_limit = 9"), "{text}");
}

#[gpui::test]
fn a_broken_file_keeps_the_last_good_settings_and_is_not_overwritten(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.paths.config_file();
    std::fs::write(&file, "[general]\nhistory_limit = 7\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 7);

    let broken = "[general\nhistory_limit = ";
    std::fs::write(&file, broken).unwrap();
    wait_until(cx, "the file to be seen as broken", |cx| {
        cx.update(|cx| rig.settings.read(cx).load_error().is_some())
    });
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 11, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    settle(cx);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    assert_eq!(history_limit(&rig, cx), 11);
}

#[gpui::test]
fn the_daemons_own_save_does_not_come_back_as_an_edit(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let seen = Rc::new(std::cell::Cell::new(0));
    let counter = seen.clone();
    let _sub =
        cx.update(|cx| cx.subscribe(&rig.settings, move |_, _, _| counter.set(counter.get() + 1)));
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 5, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    settle(cx);
    assert_eq!(seen.get(), 1);
    assert_eq!(history_limit(&rig, cx), 5);
}

#[gpui::test]
fn overlay_and_keep_previews_changes_reach_the_live_deck(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(
                |c| {
                    c.overlay.card_size = "large".into();
                    c.overlay.position = "top-left".into();
                    c.overlay.dismiss_delay = 9;
                    c.capture.keep_in_deck_until_saved = !c.capture.keep_in_deck_until_saved;
                },
                cx,
            )
        });
    });
    cx.update(|cx| {
        let cfg = rig.settings.read(cx).config().clone();
        let policy = rig.deck.read(cx).policy();
        assert_eq!(
            policy,
            DeckPolicy::from_settings(&cfg.overlay, &cfg.capture)
        );
        assert_eq!(policy.delay, Some(std::time::Duration::from_secs(9)));
        assert_eq!(
            rig.deck_surface.settings(),
            DeckSettings::from_settings(&cfg.overlay)
        );
        assert_ne!(rig.deck_surface.settings(), DeckSettings::default());
    });
}

#[gpui::test]
fn a_new_history_limit_reaches_the_live_history(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let limits = Rc::new(std::cell::Cell::new(0));
    let counter = limits.clone();
    let _sub = cx.update(|cx| {
        cx.subscribe(&rig.history, move |_, e: &HistoryEvent, _| {
            if *e == HistoryEvent::LimitChanged {
                counter.set(counter.get() + 1);
            }
        })
    });
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 3, cx))
    });
    assert_eq!(limits.get(), 1);
}

#[gpui::test]
fn a_broken_file_raises_a_toast_with_settings_closed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    std::fs::write(rig.paths.config_file(), "[general\nhistory_limit = ").unwrap();
    wait_until(cx, "the toast", |_| !sink.shown().is_empty());
    let shown = sink.shown();
    assert_eq!(shown[0].title, "Settings file has an error");
    assert!(
        shown[0]
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Line 1"))
    );
}

#[gpui::test]
fn a_failed_save_raises_a_toast_with_settings_closed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    std::fs::remove_dir_all(rig.paths.config_file().parent().unwrap()).unwrap();
    std::fs::write(rig.paths.config_file().parent().unwrap(), "not a directory").unwrap();
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.play_sound = false, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    wait_until(cx, "the toast", |_| !sink.shown().is_empty());
    assert_eq!(sink.shown()[0].title, "Couldn’t save settings");
}
