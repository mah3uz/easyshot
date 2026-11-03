#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::settings_model::{SAVE_DEBOUNCE, SettingsModel};
use es_backend::{BindHelper, BindSyntax, Caps, Env, Probe, hyprland, niri, wlroots};
use es_shell::settings::{BindStatus, ClipboardSink, Page, SettingsDeps, SettingsWindow, Toggle};
use es_store::{ConfigFile, Paths};
use es_testkit::Headless;
use es_ui::{
    assets::Assets,
    components::gallery,
    theme,
    tokens::{Appearance, ThemeMode},
};
use gpui::{
    AppContext, Entity, Keystroke, MouseButton, MouseDownEvent, MouseUpEvent, OperatingSystem,
    PlatformInput, point, px,
};
use gpui_component::Root;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Default)]
struct FakeClipboard(Rc<RefCell<Vec<String>>>);

impl ClipboardSink for FakeClipboard {
    fn copy(&self, text: String, _: &mut gpui::App) {
        self.0.borrow_mut().push(text);
    }
}

struct Fixture {
    app: Headless<Root>,
    view: Entity<SettingsWindow>,
    clipboard: FakeClipboard,
    config: PathBuf,
    paths: Paths,
    _dir: tempfile::TempDir,
}

fn fixture(caps: Caps, binds: Option<Arc<dyn BindHelper>>, prepare: impl FnOnce(&Path)) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let config = paths.config_file();
    let deps_paths = paths.clone();
    prepare(&config);
    let file = ConfigFile::load(&config).unwrap();
    let clipboard = FakeClipboard::default();
    let sink = clipboard.clone();
    let mut view = None;
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let app = Headless::open(Assets, gallery::init, w, h, |window, cx| {
        let settings = cx.new(|_| SettingsModel::new(file));
        let deps = SettingsDeps {
            settings,
            history: None,
            caps,
            binds,
            paths: deps_paths,
            clipboard: Rc::new(sink),
            outputs: vec!["DP-1".into(), "HDMI-A-1".into()],
            version: "v9.9.9".to_owned(),
        };
        let v = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        view = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    });
    let view = view.unwrap();
    let mut fx = Fixture {
        app,
        view,
        clipboard,
        config,
        paths,
        _dir: dir,
    };
    let handle = fx.app.window.into();
    fx.app
        .cx
        .update_window(handle, |_, window, _| window.activate_window())
        .unwrap();
    fx.app.settle();
    fx
}

impl Fixture {
    fn update<R>(
        &mut self,
        f: impl FnOnce(&mut SettingsWindow, &mut gpui::Context<SettingsWindow>) -> R,
    ) -> R {
        let view = self.view.clone();
        let r = self.app.cx.update(|cx| view.update(cx, f));
        self.app.settle();
        r
    }

    fn read<R>(&mut self, f: impl FnOnce(&SettingsWindow, &gpui::App) -> R) -> R {
        let view = self.view.clone();
        self.app.cx.update(|cx| f(view.read(cx), cx))
    }

    fn advance(&mut self, by: Duration) {
        self.app.cx.advance_clock(by);
        self.app.settle();
    }

    fn send(&mut self, event: PlatformInput) {
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .unwrap();
        self.app.settle();
    }

    fn click(&mut self, (x, y): (f32, f32)) {
        let position = point(px(x), px(y));
        self.send(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }));
        self.send(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }));
    }

    /// Focus starts the field recording, as a click on it does.
    fn record(&mut self, id: &str, keystroke: &str) {
        let field = self.read(|v, _| {
            v.shortcut_rows()
                .iter()
                .find(|r| r.entry.id == id)
                .map(|r| r.field.clone())
                .unwrap()
        });
        let keystroke = Keystroke::parse_with_os(keystroke, OperatingSystem::Linux).unwrap();
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                let focus = gpui::Focusable::focus_handle(field.read(cx), cx);
                window.focus(&focus, cx);
            })
            .unwrap();
        self.app.settle();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .unwrap();
        self.app.settle();
    }

    fn shot(&mut self, name: &str) {
        self.app.screenshot(name);
    }
}

fn hyprland_binds(dir: &Path) -> Arc<dyn BindHelper> {
    Arc::new(hyprland::HyprBinds {
        ipc: hyprland::Ipc::new(dir),
        syntax: BindSyntax::HyprLua,
        config: None,
        home: None,
    })
}

fn probe(vars: [(&str, &str); 1]) -> Probe {
    Probe {
        env: Env::from_vars(vars),
        ..Probe::default()
    }
}

fn sway_binds() -> Arc<dyn BindHelper> {
    let sway = (wlroots::BINDS.build)(&probe([(wlroots::SOCKET_ENV, "/nonexistent/sway.sock")]));
    Arc::from(sway.ok().unwrap())
}

fn niri_binds(config: &Path) -> Arc<dyn BindHelper> {
    let niri = (niri::BINDS.build)(&probe([("NIRI_CONFIG", config.to_str().unwrap())]));
    Arc::from(niri.ok().unwrap())
}

#[test]
fn a_burst_of_toggles_writes_the_config_once_after_the_quiet_period() {
    let mut fx = fixture(Caps::empty(), None, |_| {});
    for _ in 0..5 {
        fx.update(|v, cx| {
            let on = Toggle::PlaySound.get(&v.config(cx));
            v.set_toggle(Toggle::PlaySound, !on, cx)
        });
        fx.advance(Duration::from_millis(100));
    }
    assert!(
        !Toggle::PlaySound.get(&fx.read(|v, cx| v.config(cx))),
        "the change applies before it is saved"
    );
    assert!(!fx.config.exists(), "nothing is written during the burst");
    fx.advance(SAVE_DEBOUNCE - Duration::from_millis(101));
    assert!(
        !fx.config.exists(),
        "nothing is written before the quiet period ends"
    );
    fx.advance(Duration::from_millis(2));
    let written = std::fs::metadata(&fx.config).unwrap().modified().unwrap();
    let saved = ConfigFile::load(&fx.config).unwrap();
    assert!(!saved.config().general.play_sound);
    fx.advance(Duration::from_secs(2));
    let later = std::fs::metadata(&fx.config).unwrap().modified().unwrap();
    assert_eq!(written, later, "one write per burst");
}

#[test]
fn a_config_that_fails_to_parse_shows_the_error_and_is_never_overwritten() {
    let broken = "[general]\nplay_sound = maybe\n";
    let mut fx = fixture(Caps::empty(), None, |path| {
        std::fs::write(path, broken).unwrap()
    });
    let problem = fx
        .read(|v, _| v.problem().cloned())
        .expect("the error is shown");
    assert_eq!(problem.line, 2);
    fx.update(|v, cx| v.show(Page::General, cx));
    fx.shot("settings-config-error");
    fx.update(|v, cx| v.set_toggle(Toggle::CopyAfterSave, false, cx));
    fx.advance(Duration::from_secs(2));
    assert_eq!(std::fs::read_to_string(&fx.config).unwrap(), broken);
}

#[test]
fn the_setup_checklist_follows_the_backend_caps() {
    let mut fx = fixture(
        Caps::CAPTURE_OUTPUT | Caps::RECORDER | Caps::SHORTCUT_TRIGGERS,
        None,
        |_| {},
    );
    let list = fx.read(|v, _| v.setup_checklist());
    let get = |title: &str| list.iter().find(|(t, _)| t == title).unwrap().1;
    assert!(get("Screen capture"));
    assert!(get("Screen recording"));
    assert!(get("Global shortcuts"), "either shortcut capability counts");
    assert!(!get("Window capture"));
    assert!(!get("Selection overlay"));
    assert!(!get("Text recognition"));
}

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
fn the_sidebar_switches_pages_by_pointer() {
    let mut fx = fixture(Caps::empty(), None, |_| {});
    for page in [Page::Shortcuts, Page::About, Page::Capture, Page::Home] {
        fx.click(page.sidebar_row_centre());
        assert_eq!(fx.read(|v, _| v.page()), page);
    }
}

#[test]
fn every_page_in_dark_and_light() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    std::fs::write(&config, "binds {\n    Mod+Shift+3 { spawn \"grim\"; }\n}\n").unwrap();
    let caps = Caps::LAYER_SHELL | Caps::CAPTURE_OUTPUT | Caps::RECORDER | Caps::CLIPBOARD_NO_FOCUS;
    let mut fx = fixture(caps, Some(niri_binds(&config)), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("capture-fullscreen", "super-shift-3");
    fx.update(|v, cx| v.check_binds(cx));
    let view = fx.view.clone();
    fx.app.until("the check", |cx| {
        view.read(cx).bind_status("capture-fullscreen").is_some()
    });
    for (mode, name) in [(Appearance::Dark, "dark"), (Appearance::Light, "light")] {
        let theme_mode = if mode == Appearance::Dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        fx.app.cx.update(|cx| theme::set_mode(theme_mode, cx));
        for page in Page::ALL {
            fx.update(|v, cx| v.show(page, cx));
            fx.shot(&format!("settings-{page:?}-{name}").to_lowercase());
        }
    }
}

#[test]
fn about_shows_the_full_colour_logo() {
    let mut fx = fixture(Caps::empty(), None, |_| {});
    fx.update(|v, cx| v.show(Page::About, cx));
    let image = fx.app.screenshot("settings-about-logo");
    let colourful: std::collections::BTreeSet<(u8, u8, u8)> = image
        .pixels()
        .filter(|p| p.0[0].max(p.0[1]).max(p.0[2]) - p.0[0].min(p.0[1]).min(p.0[2]) > 60)
        .map(|p| (p.0[0] >> 5, p.0[1] >> 5, p.0[2] >> 5))
        .collect();
    assert!(colourful.len() >= 12, "{} colours", colourful.len());
}

fn global_keys(fx: &mut Fixture) -> std::collections::BTreeMap<String, String> {
    fx.read(|v, cx| v.config(cx).global_shortcuts.clone())
}

#[test]
fn global_keys_are_saved_in_the_config_and_come_back_after_a_restart() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("capture-window", "super-shift-w");
    assert_eq!(
        global_keys(&mut fx)
            .get("capture-window")
            .map(String::as_str),
        Some("Super+Shift+W")
    );
    fx.advance(SAVE_DEBOUNCE);
    let saved = std::fs::read_to_string(&fx.config).unwrap();
    assert!(saved.contains("[global_shortcuts]"), "{saved}");

    let mut again = fixture(Caps::BIND_CHECK, Some(sway_binds()), |config| {
        std::fs::write(config, saved).unwrap();
    });
    let key = again.read(|v, cx| {
        v.shortcut_rows()
            .iter()
            .find(|r| r.entry.id == "capture-window")
            .and_then(|r| r.field.read(cx).trigger().map(|t| t.label()))
    });
    assert_eq!(key.as_deref(), Some("Super+Shift+W"));
    assert!(
        again
            .read(|v, _| v.snippet())
            .unwrap()
            .contains("bindsym Mod4+Shift+w exec easyshot capture window"),
        "the restored key is bound again"
    );
}

#[test]
fn clearing_a_global_key_drops_it_from_the_config_and_the_binds_file() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    let file = fx.paths.binds_file("binds.conf");
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .all(|l| l.starts_with('#')),
        "the file exists, empty of binds, before any key is chosen"
    );
    fx.record("capture-window", "super-shift-w");
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("capture window")
    );
    fx.record("capture-region", "printscreen");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("bindsym Print exec easyshot capture region"),
        "{text}"
    );
    assert!(text.contains("capture window"), "{text}");

    fx.record("capture-window", "delete");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("capture window"), "{text}");
    assert!(text.contains("capture region"), "{text}");
    assert!(!global_keys(&mut fx).contains_key("capture-window"));
    let names: Vec<_> = std::fs::read_dir(fx.paths.config_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".easyshot-tmp"))
        .collect();
    assert!(
        names.is_empty(),
        "the rewrite leaves no temp file: {names:?}"
    );
}

fn wait_for_include(fx: &mut Fixture) -> Result<bool, String> {
    let view = fx.view.clone();
    fx.app
        .until("the include check", |cx| view.read(cx).included().is_some());
    fx.read(|v, _| v.included().cloned()).unwrap()
}

#[test]
fn the_include_check_reads_found_not_found_and_a_sourced_file() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    let mut fx = fixture(Caps::BIND_CHECK, Some(niri_binds(&config)), |_| {});
    let line = fx.read(|v, _| v.include_line()).unwrap();
    assert!(line.starts_with("include optional=true \""), "{line}");

    std::fs::write(&config, "binds { }\n").unwrap();
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    assert_eq!(wait_for_include(&mut fx), Ok(false));

    std::fs::write(&config, format!("binds {{ }}\n{line}\n")).unwrap();
    fx.update(|v, cx| v.check_binds(cx));
    assert_eq!(wait_for_include(&mut fx), Ok(true));

    std::fs::write(&config, "include \"keys.kdl\"\n").unwrap();
    std::fs::write(dir.path().join("keys.kdl"), format!("{line}\n")).unwrap();
    fx.update(|v, cx| v.check_binds(cx));
    assert_eq!(wait_for_include(&mut fx), Ok(true));

    std::fs::remove_file(&config).unwrap();
    fx.update(|v, cx| v.check_binds(cx));
    assert!(
        wait_for_include(&mut fx).is_err(),
        "no config is a problem, not a miss"
    );
}

#[test]
fn reload_asks_the_compositor_and_shows_when_it_cannot() {
    let mut fx = fixture(Caps::BIND_CHECK, Some(sway_binds()), |_| {});
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.update(|v, cx| v.reload_compositor(cx));
    let view = fx.view.clone();
    fx.app
        .until("the reload", |cx| view.read(cx).reloaded().is_some());
    assert!(fx.read(|v, _| v.reloaded().cloned()).unwrap().is_err());
}

#[test]
fn the_shortcuts_page_shows_the_include_line_in_light_and_dark() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.kdl");
    let mut fx = fixture(Caps::BIND_CHECK, Some(niri_binds(&config)), |_| {});
    let line = fx.read(|v, _| v.include_line()).unwrap();
    std::fs::write(&config, format!("binds {{ }}\n{line}\n")).unwrap();
    fx.update(|v, cx| v.show(Page::Shortcuts, cx));
    fx.record("capture-region", "printscreen");
    assert_eq!(wait_for_include(&mut fx), Ok(true));
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        fx.app.cx.update(|cx| theme::set_mode(mode, cx));
        fx.update(|_, cx| cx.notify());
        fx.shot(&format!("settings-shortcuts-include-{name}"));
    }
}
