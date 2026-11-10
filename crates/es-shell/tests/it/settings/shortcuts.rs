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

#[test]
fn long_chords_and_conflicts_at_the_default_and_minimum_width() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    std::fs::write(
        &config,
        "binds {\n    Mod+Ctrl+Alt+Shift+S { spawn \"sh\" \"-c\" \"grim -g \\\"$(slurp -d)\\\" - | swappy -f - -o ~/Pictures/latest.png\"; }\n    Mod+Shift+3 { spawn \"grim\"; }\n}\n",
    )
    .unwrap();
    for (size, name) in [
        (es_shell::settings::WINDOW_SIZE, "default"),
        (es_shell::settings::MIN_SIZE, "minimum"),
    ] {
        long_chords_at(size, name, &config);
    }
}

fn long_chords_at(size: (f32, f32), size_name: &str, config: &Path) {
    let mut fx = fixture_sized(size, Caps::BIND_CHECK, Some(niri_binds(config)), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("capture-bar", "super-ctrl-alt-shift-s");
    fx.record("gallery", "super-ctrl-alt-shift-backspace");
    fx.record("capture-fullscreen", "super-shift-3");
    fx.update(|v, cx| v.check_binds(cx));
    let view = fx.view.clone();
    fx.app.until("the check", |cx| {
        view.read(cx).bind_status("capture-bar").is_some()
    });
    assert!(matches!(
        fx.read(|v, _| v.bind_status("capture-bar").cloned()),
        Some(BindStatus::Conflict(_))
    ));
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        fx.app.cx.update(|cx| theme::set_mode(mode, cx));
        fx.update(|_, cx| cx.notify());
        fx.shot(&format!("settings-shortcuts-long-{size_name}-{name}"));
    }
}

#[test]
fn keys_in_the_written_binds_file_count_as_bound_when_the_config_has_none() {
    use std::io::{Read, Write};
    let dir = tempfile::tempdir().unwrap();
    let socket = std::os::unix::net::UnixListener::bind(dir.path().join(".socket.sock")).unwrap();
    let bind = |key: &str, arg: &str, dispatcher: &str| {
        format!(
            r#"{{"modmask": 65, "submap": "", "key": "{key}", "keycode": 0, "dispatcher": "{dispatcher}", "arg": "{arg}"}}"#
        )
    };
    let reply = format!(
        "[{}]",
        [
            bind("4", "85", "HL.Dispatcher(exec_cmd)"),
            bind("3", "86", "HL.Dispatcher(exec_cmd)"),
            bind("W", "grim", "exec"),
        ]
        .join(",")
    );
    std::thread::spawn(move || {
        for stream in socket.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut request = [0u8; 64];
            let _ = stream.read(&mut request);
            stream.write_all(reply.as_bytes()).ok();
        }
    });
    let main = dir.path().join("hyprland.lua");
    let binds = Arc::new(hyprland::HyprBinds {
        ipc: hyprland::Ipc::new(dir.path()),
        syntax: BindSyntax::HyprLua,
        config: Some(main.clone()),
        home: None,
    });
    let mut fx = fixture(Caps::BIND_CHECK, Some(binds), |config| {
        let config_dir = config.parent().unwrap();
        std::fs::write(
            &main,
            format!("pcall(require, \"{}/binds\")\n", config_dir.display()),
        )
        .unwrap();
        std::fs::write(
            config_dir.join("binds.lua"),
            "-- header\n\
             hl.bind(\"SUPER + SHIFT + 4\", hl.dsp.exec_cmd(\"easyshot capture region\"))\n\
             hl.bind(\"SUPER + SHIFT + 3\", hl.dsp.exec_cmd(\"easyshot capture fullscreen\"))\n\
             hl.bind(\"SUPER + SHIFT + W\", hl.dsp.exec_cmd(\"easyshot capture window\"))\n",
        )
        .unwrap();
    });
    assert!(
        ConfigFile::load(&fx.config)
            .unwrap()
            .config()
            .global_shortcuts
            .is_empty()
    );
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    let key = |fx: &mut Fixture, id: &str| {
        fx.read(|v, cx| {
            v.shortcut_rows()
                .iter()
                .find(|r| r.entry.id == id)
                .and_then(|r| r.field.read(cx).trigger().map(|t| t.label()))
        })
    };
    assert_eq!(
        key(&mut fx, "capture-region").as_deref(),
        Some("Super+Shift+4")
    );
    assert_eq!(
        key(&mut fx, "capture-window").as_deref(),
        Some("Super+Shift+W")
    );
    fx.update(|v, cx| v.check_binds(cx));
    let view = fx.view.clone();
    fx.app.until("the check", |cx| {
        view.read(cx).bind_status("capture-region").is_some()
    });
    let status = |fx: &mut Fixture, id: &str| fx.read(|v, _| v.bind_status(id).cloned());
    assert_eq!(status(&mut fx, "capture-region"), Some(BindStatus::Bound));
    assert_eq!(
        status(&mut fx, "capture-fullscreen"),
        Some(BindStatus::Bound)
    );
    assert!(
        matches!(status(&mut fx, "capture-window"), Some(BindStatus::Conflict(c)) if c.contains("grim"))
    );
}
