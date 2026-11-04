#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::gallery_support::{FakeFiles, SCREENSHOTS, VIDEOS, library, videos};
use es_app::history::History;
use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_shell::home::{self, HomeDeps, HomeEvent, HomeWindow, OpenPicker, Tab};
use es_shell::settings::{ClipboardSink, Page, SettingsDeps};
use es_store::ConfigFile;
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{
    AppContext, Entity, ExternalPaths, FileDropEvent, MouseButton, MouseDownEvent, MouseUpEvent,
    PlatformInput, Task, point, px,
};
use gpui_component::Root;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use tempfile::TempDir;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

struct Rig {
    opened: Rc<RefCell<Vec<PathBuf>>>,
    app: Headless<Root>,
    home: Entity<HomeWindow>,
    _dir: TempDir,
}

fn rig(mode: ThemeMode, tab: Tab) -> Rig {
    rig_with(mode, tab, None)
}

fn rig_with(mode: ThemeMode, tab: Tab, open_picker: Option<OpenPicker>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = library(dir.path(), SCREENSHOTS);
    videos(&paths, VIDEOS);
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
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        w,
        h,
        move |window, cx| {
            let history = cx.new(|_| History::open(&paths, 0).unwrap());
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
                    input_access: Rc::new(|| es_model::recording::InputAccess::Granted),
                },
                open_picker,
            };
            let (root, home) = home::root(deps, tab, window, cx);
            *sink.borrow_mut() = Some(home);
            root
        },
    );
    let home = made.borrow_mut().take().unwrap();
    let opened = Rc::new(RefCell::new(Vec::new()));
    let log = opened.clone();
    let mut app = app;
    app.cx.update(|cx| {
        cx.subscribe(&home, move |_, HomeEvent::Open(file), _| {
            log.borrow_mut().push(file.clone())
        })
        .detach();
    });
    Rig {
        opened,
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

fn picking(file: Option<&str>) -> OpenPicker {
    let file = file.map(PathBuf::from);
    Rc::new(move |_| Task::ready(file.clone()))
}

fn open_button_x() -> f32 {
    home::WINDOW_SIZE.0 - 50.
}

#[test]
fn the_open_button_hands_the_picked_file_to_the_daemon() {
    let mut rig = rig_with(
        ThemeMode::Dark,
        Tab::Gallery,
        Some(picking(Some("/x/clip.mp4"))),
    );
    rig.click(open_button_x(), home::TAB_BAR_HEIGHT / 2.);
    assert_eq!(*rig.opened.borrow(), [PathBuf::from("/x/clip.mp4")]);
}

#[test]
fn cancelling_the_picker_opens_nothing() {
    let mut rig = rig_with(ThemeMode::Dark, Tab::Gallery, Some(picking(None)));
    rig.click(open_button_x(), home::TAB_BAR_HEIGHT / 2.);
    assert!(rig.opened.borrow().is_empty());
}

#[test]
fn without_a_file_chooser_there_is_no_open_button() {
    let mut rig = rig(ThemeMode::Dark, Tab::Gallery);
    rig.click(open_button_x(), home::TAB_BAR_HEIGHT / 2.);
    assert!(rig.opened.borrow().is_empty());
    let home = rig.home.clone();
    rig.app
        .cx
        .update(|cx| home.update(cx, |h, cx| h.choose_file(cx)));
    rig.app.settle();
    assert!(rig.opened.borrow().is_empty());
}

#[test]
fn dropping_a_file_on_the_window_opens_it() {
    let mut rig = rig(ThemeMode::Dark, Tab::Gallery);
    let at = point(px(400.), px(300.));
    let mut paths = ExternalPaths::default();
    paths.0.push(PathBuf::from("/x/shot.png"));
    let handle = rig.app.window.into();
    for event in [
        FileDropEvent::Entered {
            position: at,
            paths,
        },
        FileDropEvent::Pending { position: at },
        FileDropEvent::Submit { position: at },
    ] {
        rig.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(PlatformInput::FileDrop(event), cx);
            })
            .unwrap();
        rig.app.settle();
    }
    assert_eq!(*rig.opened.borrow(), [PathBuf::from("/x/shot.png")]);
}

fn pictures(mode: ThemeMode, tag: &str) {
    let mut rig = rig_with(mode, Tab::Gallery, Some(picking(None)));
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
