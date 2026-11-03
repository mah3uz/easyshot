#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::gallery_support::{FakeFiles, SCREENSHOTS, VIDEOS, library, video_entries};
use es_app::history::History;
use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_shell::home::{self, HomeDeps, HomeWindow, Tab};
use es_shell::settings::{ClipboardSink, Page, SettingsDeps};
use es_store::ConfigFile;
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Entity, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point, px,
};
use gpui_component::Root;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use tempfile::TempDir;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

struct Rig {
    app: Headless<Root>,
    home: Entity<HomeWindow>,
    _dir: TempDir,
}

fn rig(mode: ThemeMode, tab: Tab) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = library(dir.path(), SCREENSHOTS);
    let videos = video_entries(dir.path(), VIDEOS);
    let config = ConfigFile::load(&paths.config_file()).unwrap();
    let files = Arc::new(FakeFiles {
        trash_dir: dir.path().join("Trash"),
        ..FakeFiles::default()
    });
    let made = Rc::new(RefCell::new(None));
    let sink = made.clone();
    let (w, h) = home::WINDOW_SIZE;
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::hello::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        w,
        h,
        move |window, cx| {
            let history = cx.new(|_| History::open(&paths, 0).unwrap());
            history.update(cx, |h, cx| {
                for entry in videos {
                    h.add(entry, cx).unwrap();
                }
            });
            let deps = HomeDeps {
                history: history.clone(),
                paths: paths.clone(),
                files,
                settings: SettingsDeps {
                    settings: cx.new(|_| SettingsModel::new(config)),
                    history: Some(history),
                    caps: Caps::all(),
                    binds: None,
                    paths: paths.clone(),
                    clipboard: Rc::new(NoClipboard),
                    outputs: vec!["DP-1".into()],
                    version: "v9.9.9".to_owned(),
                },
            };
            let (root, home) = home::root(deps, tab, window, cx);
            *sink.borrow_mut() = Some(home);
            root
        },
    );
    let home = made.borrow_mut().take().unwrap();
    Rig {
        app,
        home,
        _dir: dir,
    }
}

impl Rig {
    fn tab(&mut self) -> (Tab, Page) {
        let home = self.home.clone();
        self.app.cx.update(|cx| {
            let h = home.read(cx);
            (h.tab(), h.settings().read(cx).page())
        })
    }

    fn click(&mut self, x: f32, y: f32) {
        let position = point(px(x), px(y));
        let handle = self.app.window.into();
        for event in [
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Default::default(),
                click_count: 1,
            }),
        ] {
            self.app
                .cx
                .update_window(handle, |_, window, cx| {
                    window.dispatch_event(event, cx);
                })
                .unwrap();
            self.app.settle();
        }
    }

    fn picture(&mut self, name: &str) {
        let image = self.app.screenshot(name);
        if let Some(dir) = std::env::var_os("EASYSHOT_PICTURES") {
            std::fs::create_dir_all(&dir).unwrap();
            image
                .save(Path::new(&dir).join(format!("{name}.png")))
                .unwrap();
        }
    }

    fn wait_for_thumbnails(&mut self) {
        let home = self.home.clone();
        let gallery = self.app.cx.update(|cx| home.read(cx).gallery().clone());
        self.app
            .until("thumbnails", |cx| gallery.read(cx).thumbnails_ready() >= 18);
    }
}

#[test]
fn clicking_a_tab_switches_between_the_gallery_and_settings() {
    let mut rig = rig(ThemeMode::Dark, Tab::Gallery);
    assert_eq!(rig.tab().0, Tab::Gallery);
    let (w, _) = home::WINDOW_SIZE;
    let y = home::TAB_BAR_HEIGHT / 2.;
    rig.click(w / 2. + 50., y);
    assert_eq!(rig.tab().0, Tab::Settings);
    rig.click(w / 2. - 50., y);
    assert_eq!(rig.tab().0, Tab::Gallery);
}

#[test]
fn show_settings_with_a_page_lands_on_that_page() {
    let mut rig = rig(ThemeMode::Dark, Tab::Gallery);
    let home = rig.home.clone();
    rig.app.cx.update(|cx| {
        home.update(cx, |h, cx| h.show_settings(Some(Page::Overlay), cx));
    });
    assert_eq!(rig.tab(), (Tab::Settings, Page::Overlay));
    rig.app.cx.update(|cx| {
        home.update(cx, |h, cx| h.show(Tab::Gallery, cx));
        home.update(cx, |h, cx| h.show_settings(None, cx));
    });
    assert_eq!(rig.tab(), (Tab::Settings, Page::Overlay));
}

fn pictures(mode: ThemeMode, tag: &str) {
    let mut rig = rig(mode, Tab::Gallery);
    rig.wait_for_thumbnails();
    rig.picture(&format!("home-gallery-{tag}"));
    let mut rig = self::rig(mode, Tab::Settings);
    rig.picture(&format!("home-settings-{tag}"));
}

#[test]
fn home_pictures_dark() {
    pictures(ThemeMode::Dark, "dark");
}

#[test]
fn home_pictures_light() {
    pictures(ThemeMode::Light, "light");
}
