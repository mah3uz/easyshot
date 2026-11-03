#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_shell::settings::{ClipboardSink, Page, SettingsDeps, SettingsWindow};
use es_store::{ConfigFile, Paths};
use es_testkit::Headless;
use es_ui::{
    assets::Assets,
    components::gallery,
    motion::{PAGE_CHANGE, set_reduce_motion},
};
use gpui::{AppContext, Entity};
use gpui_component::Root;
use std::rc::Rc;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

fn open(dir: &std::path::Path) -> (Headless<Root>, Entity<SettingsWindow>) {
    let paths = Paths::rooted(dir);
    paths.ensure_all().unwrap();
    let file = ConfigFile::load(&paths.config_file()).unwrap();
    let mut view = None;
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let app = Headless::open(Assets, gallery::init, w, h, |window, cx| {
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
    app.screenshot(name)
}

#[test]
fn a_page_rises_and_fades_in_over_its_token_and_reduce_motion_shows_it_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, view) = open(dir.path());
    let show = |app: &mut Headless<Root>, page: Page| {
        app.cx
            .update(|cx| view.update(cx, |v, cx| v.show(page, cx)));
        app.settle();
    };

    show(&mut app, Page::Capture);
    assert_eq!(
        app.cx.update(|cx| view.read(cx).page()),
        Page::Capture,
        "the page changes at once; only its drawing moves"
    );
    let start = frame(&mut app, "settings-page-change-0");
    app.cx.advance_clock(PAGE_CHANGE.duration / 2);
    let mid = frame(&mut app, "settings-page-change-1-mid");
    app.cx.advance_clock(PAGE_CHANGE.duration / 2);
    let end = frame(&mut app, "settings-page-change-2-end");
    assert!(start != mid && mid != end && start != end);

    show(&mut app, Page::Home);
    app.cx.advance_clock(PAGE_CHANGE.duration);
    app.cx.update(|cx| set_reduce_motion(true, cx));
    show(&mut app, Page::Capture);
    assert!(
        frame(&mut app, "settings-page-change-reduced") == end,
        "with Reduce Motion the new page shows whole"
    );
}
