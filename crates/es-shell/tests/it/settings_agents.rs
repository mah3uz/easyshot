#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::agent_access::{AgentAccess, AgentAccessHandle, Grant};
use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_model::ActionRecord;
use es_model::recording::InputAccess;
use es_protocol::agent::GrantSpan;
use es_shell::settings::{ClipboardSink, Page, SettingsDeps, SettingsWindow};
use es_store::{ConfigFile, Paths};
use es_testkit::Headless;
use es_ui::{assets::Assets, theme, tokens::ThemeMode};
use gpui::{AppContext, Entity};
use gpui_component::Root;
use std::rc::Rc;
use time::OffsetDateTime;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: String, _: &mut gpui::App) {}
}

struct Fixture {
    app: Headless<Root>,
    view: Entity<SettingsWindow>,
    access: Entity<AgentAccess>,
    _dir: tempfile::TempDir,
}

const NOON: i64 = 1_793_793_600;

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let file = ConfigFile::load(&paths.config_file()).unwrap();
    let agent_file = es_store::agent::open(&paths);
    let now = OffsetDateTime::from_unix_timestamp(NOON).unwrap();
    let (mut view, mut access) = (None, None);
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let deps_paths = paths.clone();
    let app = Headless::open(Assets, es_ui::init, w, h * 1.6, |window, cx| {
        let a = cx.new(|_| AgentAccess::new(agent_file, Rc::new(move || now)));
        cx.set_global(AgentAccessHandle(a.clone()));
        access = Some(a);
        let deps = SettingsDeps {
            settings: cx.new(|_| SettingsModel::new(file)),
            history: None,
            caps: Caps::empty(),
            binds: None,
            paths: deps_paths,
            clipboard: Rc::new(NoClipboard),
            outputs: Vec::new(),
            version: "v9.9.9".to_owned(),
            input_access: Rc::new(|| InputAccess::Granted),
        };
        let v = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        view = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    });
    let mut fx = Fixture {
        app,
        view: view.unwrap(),
        access: access.unwrap(),
        _dir: dir,
    };
    fx.app
        .cx
        .update(|cx| es_ui::motion::set_reduce_motion(true, cx));
    let view = fx.view.clone();
    fx.app
        .cx
        .update(|cx| view.update(cx, |v, cx| v.show(Page::Agents, cx)));
    fx.app.settle();
    fx
}

fn entry(minutes: i64, route: &str, target: Option<&str>, result: &str) -> ActionRecord {
    ActionRecord {
        at: OffsetDateTime::from_unix_timestamp(NOON - minutes * 60).unwrap(),
        route: route.to_owned(),
        target: target.map(str::to_owned),
        result: result.to_owned(),
        pid: Some(48213),
        comm: Some("claude".to_owned()),
    }
}

#[test]
fn the_log_and_the_grant_show_as_they_change_and_revoke_ends_the_grant() {
    let mut fx = fixture();
    fx.app.screenshot("settings-agents-empty");
    let access = fx.access.clone();
    fx.app.cx.update(|cx| {
        access.update(cx, |a, cx| {
            a.record(entry(9, "agent/list-outputs", None, "access_off"), 50, cx);
            a.record(
                entry(4, "agent/capture", Some("output DP-1"), "access_denied"),
                50,
                cx,
            );
            a.record(entry(2, "agent/capture", Some("output DP-1"), "ok"), 50, cx);
            a.record(entry(1, "agent/ocr", Some("window firefox"), "ok"), 50, cx);
            a.allow(GrantSpan::Hour, cx);
        })
    });
    fx.app.settle();
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let view = fx.view.clone();
        fx.app.cx.update(|cx| {
            theme::set_mode(mode, cx);
            view.update(cx, |_, cx| cx.notify());
        });
        fx.app
            .screenshot(&format!("settings-agents-granted-{name}"));
    }
    let grant = fx.app.cx.update(|cx| access.read(cx).grant());
    assert!(matches!(grant, Some(Grant::Until(_))));
    assert_eq!(fx.app.cx.update(|cx| access.read(cx).log().len()), 4);
    fx.app
        .cx
        .update(|cx| access.update(cx, |a, cx| a.revoke(cx)));
    fx.app.settle();
    assert_eq!(fx.app.cx.update(|cx| access.read(cx).grant()), None);
}
