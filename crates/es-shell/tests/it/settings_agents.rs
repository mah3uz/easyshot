#![allow(clippy::unwrap_used, clippy::expect_used)]

use es_app::agent_access::{AgentAccess, AgentAccessHandle, Grant};
use es_app::settings_model::SettingsModel;
use es_backend::Caps;
use es_model::recording::InputAccess;
use es_model::{ActionRecord, AgentCfg, Consent};
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
    settings: Entity<SettingsModel>,
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
    let (mut view, mut access, mut model) = (None, None, None);
    let (w, h) = es_shell::settings::WINDOW_SIZE;
    let deps_paths = paths.clone();
    let app = Headless::open(Assets, es_ui::init, w, h * 1.6, |window, cx| {
        let a = cx.new(|_| AgentAccess::new(agent_file, Rc::new(move || now)));
        cx.set_global(AgentAccessHandle(a.clone()));
        access = Some(a);
        let settings = cx.new(|_| SettingsModel::new(file));
        model = Some(settings.clone());
        let deps = SettingsDeps {
            settings,
            history: None,
            caps: Caps::empty(),
            binds: None,
            paths: deps_paths,
            clipboard: Rc::new(NoClipboard),
            outputs: Vec::new(),
            version: "v9.9.9".to_owned(),
            input_access: Rc::new(|| InputAccess::Granted),
            shares: None,
        };
        let v = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        view = Some(v.clone());
        cx.new(|cx| Root::new(v, window, cx))
    });
    let mut fx = Fixture {
        app,
        view: view.unwrap(),
        access: access.unwrap(),
        settings: model.unwrap(),
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

fn agent_cfg(fx: &mut Fixture) -> AgentCfg {
    let settings = fx.settings.clone();
    fx.app
        .cx
        .update(|cx| settings.read(cx).config().agent.clone())
}

fn edit_view(
    fx: &mut Fixture,
    f: impl FnOnce(&mut SettingsWindow, &mut gpui::Context<SettingsWindow>),
) {
    let view = fx.view.clone();
    fx.app.cx.update(|cx| view.update(cx, f));
    fx.app.settle();
}

fn field(fx: &mut Fixture, outputs: bool) -> Entity<gpui_component::input::InputState> {
    let view = fx.view.clone();
    let (o, w) = fx.app.cx.update(|cx| view.read(cx).agent_scope_fields());
    if outputs { o } else { w }
}

fn type_into(fx: &mut Fixture, outputs: bool, text: &str) {
    let field = field(fx, outputs);
    let handle = fx.app.window.into();
    fx.app
        .cx
        .update_window(handle, |_, window, cx| {
            let focus = gpui::Focusable::focus_handle(field.read(cx), cx);
            window.focus(&focus, cx);
        })
        .unwrap();
    fx.app.settle();
    for c in text.chars() {
        let key = c.to_string();
        let stroke = gpui::Keystroke {
            modifiers: Default::default(),
            key: key.clone(),
            key_char: Some(key),
        };
        fx.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_keystroke(stroke, cx)
            })
            .unwrap();
        fx.app.settle();
    }
}

fn field_text(fx: &mut Fixture, outputs: bool) -> String {
    let field = field(fx, outputs);
    fx.app.cx.update(|cx| field.read(cx).value().to_string())
}

#[test]
fn each_control_writes_its_own_agent_setting() {
    let mut fx = fixture();
    assert_eq!(agent_cfg(&mut fx), AgentCfg::default());

    edit_view(&mut fx, |v, cx| v.set_agent_enabled(true, cx));
    assert!(agent_cfg(&mut fx).enabled);
    edit_view(&mut fx, |v, cx| v.set_agent_consent(Consent::AlwaysAsk, cx));
    assert_eq!(agent_cfg(&mut fx).consent, Consent::AlwaysAsk);
    edit_view(&mut fx, |v, cx| v.set_agent_redact(false, cx));
    assert!(!agent_cfg(&mut fx).redact_secrets);
    edit_view(&mut fx, |v, cx| v.set_agent_keep_captures(false, cx));
    assert!(!agent_cfg(&mut fx).keep_captures);

    type_into(&mut fx, true, "DP-1, HDMI-A-1, DP-1");
    assert_eq!(agent_cfg(&mut fx).allowed_outputs, ["DP-1", "HDMI-A-1"]);
    type_into(&mut fx, false, " firefox ,foot");
    assert_eq!(agent_cfg(&mut fx).allowed_windows, ["firefox", "foot"]);
    let cfg = agent_cfg(&mut fx);
    assert!(cfg.enabled && !cfg.redact_secrets && cfg.consent == Consent::AlwaysAsk);
}

#[test]
fn turning_agents_off_ends_the_grant() {
    let mut fx = fixture();
    let access = fx.access.clone();
    edit_view(&mut fx, |v, cx| v.set_agent_enabled(true, cx));
    fx.app
        .cx
        .update(|cx| access.update(cx, |a, cx| a.allow(GrantSpan::Session, cx)));
    edit_view(&mut fx, |v, cx| v.set_agent_enabled(false, cx));
    assert_eq!(fx.app.cx.update(|cx| access.read(cx).grant()), None);
}

#[test]
fn the_log_list_is_redrawn_when_an_action_is_logged() {
    let mut fx = fixture();
    let before = fx.app.screenshot("settings-agents-log-before");
    let access = fx.access.clone();
    fx.app.cx.update(|cx| {
        access.update(cx, |a, cx| {
            a.record(entry(1, "agent/capture", Some("output DP-1"), "ok"), 50, cx)
        })
    });
    fx.app.settle();
    let after = fx.app.screenshot("settings-agents-log-after");
    assert_ne!(
        before.as_raw(),
        after.as_raw(),
        "the new action is not on the page"
    );
}

#[test]
fn the_scope_fields_follow_a_config_changed_outside_the_page() {
    let mut fx = fixture();
    let settings = fx.settings.clone();
    fx.app.cx.update(|cx| {
        settings.update(cx, |m, cx| {
            m.update(
                |c| {
                    c.agent.allowed_outputs = vec!["DP-2".into(), "eDP-1".into()];
                    c.agent.allowed_windows = vec!["kitty".into()];
                },
                cx,
            )
        })
    });
    fx.app.settle();
    assert_eq!(field_text(&mut fx, true), "DP-2, eDP-1");
    assert_eq!(field_text(&mut fx, false), "kitty");
    fx.app.cx.update(|cx| {
        settings.update(cx, |m, cx| {
            m.update(|c| c.agent.allowed_outputs.clear(), cx)
        })
    });
    fx.app.settle();
    assert_eq!(field_text(&mut fx, true), "");
    assert_eq!(agent_cfg(&mut fx).allowed_windows, ["kitty"]);
}
