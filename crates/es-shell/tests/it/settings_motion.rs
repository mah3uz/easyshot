#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_shell::settings::{ClipboardSink, Page, SettingsDeps, SettingsWindow};
use es_store::{ConfigFile, Paths};
use es_testkit::Headless;
use es_ui::{
    assets::Assets,
    motion::{PAGE_CHANGE, set_reduce_motion},
    tokens::ThemeMode,
};
use gpui::{AppContext, Entity};
use gpui_component::Root;
use std::rc::Rc;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

fn open(dir: &std::path::Path, mode: ThemeMode) -> (Headless<Root>, Entity<SettingsWindow>) {
    let paths = Paths::rooted(dir);
    paths.ensure_all().unwrap();
    let file = ConfigFile::load(&paths.config_file()).unwrap();
    let mut view = None;
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let init = move |cx: &mut gpui::App| {
        es_ui::init(cx);
        es_ui::theme::set_mode(mode, cx);
    };
    let app = Headless::open(Assets, init, w, h, |window, cx| {
        let deps = SettingsDeps {
            settings: cx.new(|_| SettingsModel::new(file)),
            history: None,
            caps: Caps::empty(),
            binds: None,
            paths,
            clipboard: Rc::new(NoClipboard),
            outputs: vec!["DP-1".into()],
            version: "v9.9.9".to_owned(),
            input_access: Rc::new(|| es_model::recording::InputAccess::Granted),
        };
        let v = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        view = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    });
    (app, view.unwrap())
}

fn frame(app: &mut Headless<Root>, name: &str) -> image::RgbaImage {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .unwrap();
    let image = app.screenshot(name);
    if let Some(dir) = std::env::var_os("EASYSHOT_PICTURES") {
        std::fs::create_dir_all(&dir).unwrap();
        image
            .save(std::path::Path::new(&dir).join(format!("{name}.png")))
            .unwrap();
    }
    image
}

/// The frame right of the sidebar, whose selection moves at once.
fn pane(frame: &image::RgbaImage) -> image::RgbaImage {
    let left = (es_shell::settings::SIDEBAR_WIDTH * frame.width() as f32
        / es_shell::settings::WINDOW_SIZE.0) as u32;
    image::imageops::crop_imm(frame, left, 0, frame.width() - left, frame.height()).to_image()
}

fn page_change(mode: ThemeMode, tag: &str) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, view) = open(dir.path(), mode);
    let show = |app: &mut Headless<Root>, page: Page| {
        app.cx
            .update(|cx| view.update(cx, |v, cx| v.show(page, cx)));
        app.settle();
    };

    let before = frame(&mut app, &format!("settings-page-change-{tag}-0-before"));
    show(&mut app, Page::Capture);
    assert_eq!(
        app.cx.update(|cx| view.read(cx).page()),
        Page::Capture,
        "the page changes at once; only its drawing moves"
    );
    let start = frame(&mut app, &format!("settings-page-change-{tag}-1-start"));
    assert!(
        pane(&start) == pane(&before),
        "the old page is still whole as the change starts, so the pane never blinks empty"
    );
    app.cx.advance_clock(PAGE_CHANGE.duration / 2);
    let mid = frame(&mut app, &format!("settings-page-change-{tag}-2-mid"));
    app.cx.advance_clock(PAGE_CHANGE.duration / 2);
    let end = frame(&mut app, &format!("settings-page-change-{tag}-3-end"));
    assert!(start != mid && mid != end && start != end);

    show(&mut app, Page::Home);
    app.cx.advance_clock(PAGE_CHANGE.duration);
    app.cx.update(|cx| set_reduce_motion(true, cx));
    show(&mut app, Page::Capture);
    assert!(
        frame(&mut app, &format!("settings-page-change-{tag}-reduced")) == end,
        "with Reduce Motion the new page shows whole"
    );
}

#[test]
fn the_old_page_fades_out_as_the_new_one_rises_and_reduce_motion_shows_it_at_once_dark() {
    page_change(ThemeMode::Dark, "dark");
}

#[test]
fn the_old_page_fades_out_as_the_new_one_rises_and_reduce_motion_shows_it_at_once_light() {
    page_change(ThemeMode::Light, "light");
}
