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
    _dir: tempfile::TempDir,
}

fn fixture(caps: Caps, binds: Option<Arc<dyn BindHelper>>, prepare: impl FnOnce(&Path)) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let config = paths.config_file();
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
    for (binds, region) in [
        (
            hyprland_binds(dir.path()),
            "hl.bind(\"SUPER + SHIFT + 4\", hl.dsp.exec_cmd(\"easyshot capture region\"))\n",
        ),
        (
            sway_binds(),
            "bindsym $mod+Shift+4 exec easyshot capture region\n",
        ),
        (
            niri_binds(&niri_config),
            "Mod+Shift+4 { spawn \"easyshot\" \"capture\" \"region\"; }\n",
        ),
    ] {
        let mut fx = fixture(Caps::BIND_CHECK, Some(binds), |_| {});
        let snippet = fx.read(|v, _| v.snippet()).unwrap();
        assert!(snippet.contains(region), "{snippet}");
        assert!(
            !snippet.contains("open-image"),
            "a route with arguments has no fixed command"
        );
        fx.update(|v, cx| v.copy_snippet(cx));
        assert_eq!(*fx.clipboard.0.borrow(), vec![snippet]);
    }
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
        snippet.contains("bindsym $mod+Shift+w exec easyshot capture window"),
        "{snippet}"
    );
    assert!(
        !stored.contains_key("capture-window"),
        "the compositor owns global keys"
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
