use super::*;

#[test]
fn each_compositor_gets_its_own_snippet_and_copy_goes_to_the_clipboard() {
    let dir = tempfile::tempdir().unwrap();
    let niri_config = dir.path().join("config.kdl");
    for (binds, binds_name, region) in [
        (
            hyprland_binds(dir.path()),
            "binds.lua",
            "hl.bind(\"SUPER + SHIFT + 4\", hl.dsp.exec_cmd(\"easyshot capture region\"))\n",
        ),
        (
            sway_binds(),
            "binds.conf",
            "bindsym Mod4+Shift+4 exec easyshot capture region\n",
        ),
        (
            niri_binds(&niri_config),
            "binds.kdl",
            "Mod+Shift+4 { spawn \"easyshot\" \"capture\" \"region\"; }\n",
        ),
    ] {
        let mut fx = fixture(Caps::BIND_CHECK, Some(binds), |_| {});
        assert_eq!(
            fx.read(|v, _| v.snippet()).as_deref(),
            Some(""),
            "no key chosen, so nothing to bind"
        );
        fx.update(|v, cx| v.show(Page::Shortcuts, cx));
        fx.record("capture-region", "super-shift-4");
        let snippet = fx.read(|v, _| v.snippet()).unwrap();
        assert_eq!(
            snippet.lines().filter(|l| l.contains("easyshot")).count(),
            1,
            "only the chosen key: {snippet}"
        );
        assert!(snippet.contains(region), "{snippet}");
        assert!(
            !snippet.contains("open-image"),
            "a route with arguments has no fixed command"
        );
        let file = fx.paths.binds_file(binds_name);
        let written = std::fs::read_to_string(&file).unwrap();
        assert!(
            written.contains(region),
            "the file holds the chosen key: {written}"
        );
        fx.update(|v, cx| v.copy_include_line(cx));
        let line = fx.read(|v, _| v.include_line()).unwrap();
        assert!(line.contains("/binds"), "{line}");
        assert_eq!(*fx.clipboard.0.borrow(), vec![line]);
    }
}

#[test]
fn stored_editor_keys_load_and_no_global_row_starts_with_a_key() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |config| {
        std::fs::write(config, "[shortcuts]\nimage-crop = \"K\"\n").unwrap();
    });
    let (crop, globals_with_key) = fx.read(|v, cx| {
        let rows = v.shortcut_rows();
        let key = |id: &str| {
            rows.iter()
                .find(|r| r.entry.id == id)
                .and_then(|r| r.field.read(cx).trigger().map(|t| t.label()))
        };
        let globals = rows
            .iter()
            .filter(|r| r.entry.scope == es_protocol::Scope::Global)
            .filter(|r| r.field.read(cx).trigger().is_some())
            .count();
        (key("image-crop"), globals)
    });
    assert_eq!(crop.as_deref(), Some("K"));
    assert_eq!(globals_with_key, 0);
}

#[test]
fn the_shortcuts_page_renders_with_no_keys_set() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.shot("settings-shortcuts-no-keys");
}

#[test]
fn check_marks_bound_actions_and_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    std::fs::write(
        &config,
        "binds {\n    Mod+Shift+4 { spawn \"easyshot\" \"capture\" \"region\"; }\n    Mod+Shift+3 { spawn \"grim\"; }\n}\n",
    )
    .unwrap();
    let mut fx = fixture(Caps::BIND_CHECK, Some(niri_binds(&config)), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("capture-fullscreen", "super-shift-3");
    fx.update(|v, cx| v.check_binds(cx));
    let view = fx.view.clone();
    fx.app.until("the check", |cx| {
        view.read(cx).bind_status("capture-region").is_some()
    });
    let (region, full, window) = fx.read(|v, _| {
        (
            v.bind_status("capture-region").cloned(),
            v.bind_status("capture-fullscreen").cloned(),
            v.bind_status("capture-window").cloned(),
        )
    });
    assert_eq!(region, Some(BindStatus::Bound));
    assert!(matches!(full, Some(BindStatus::Conflict(c)) if c.contains("grim")));
    assert_eq!(window, None);
}

#[test]
fn bind_records_a_key_and_stores_an_editor_override() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("image-crop", "k");
    let stored = fx.read(|v, cx| v.config(cx).shortcuts.get("image-crop").cloned());
    assert_eq!(stored.as_deref(), Some("K"));
    fx.advance(SAVE_DEBOUNCE);
    let saved = ConfigFile::load(&fx.config).unwrap();
    assert_eq!(
        saved
            .config()
            .shortcuts
            .get("image-crop")
            .map(String::as_str),
        Some("K")
    );

    fx.record("capture-window", "super-shift-w");
    let (snippet, stored) = fx.read(|v, cx| (v.snippet().unwrap(), v.config(cx).shortcuts.clone()));
    assert!(
        snippet.contains("bindsym Mod4+Shift+w exec easyshot capture window"),
        "{snippet}"
    );
    assert!(
        !stored.contains_key("capture-window"),
        "global keys are not editor overrides"
    );
}
