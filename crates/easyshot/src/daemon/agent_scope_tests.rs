use super::*;
use es_model::platform::{OutputId, Transform, WindowId};
use es_protocol::agent::{Geometry, OutputName};

fn output(name: &str, x: f64) -> OutputInfo {
    OutputInfo {
        id: OutputId(name.into()),
        description: String::new(),
        logical: Rect::new(x, 0., x + 1920., 1080.),
        physical: (1920, 1080),
        scale: 1.,
        transform: Transform::Normal,
        refresh_mhz: 60_000,
        focused: None,
    }
}

fn window(id: &str, app: &str, title: &str, on: &str) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: app.into(),
        title: title.into(),
        pid: None,
        logical: None,
        output: Some(OutputId(on.into())),
        workspace: None,
        floating: false,
        focus_rank: None,
    }
}

fn scope(outputs: &[&str], windows: &[&str]) -> Scope {
    Scope::new(&AgentCfg {
        allowed_outputs: outputs.iter().map(|s| (*s).to_owned()).collect(),
        allowed_windows: windows.iter().map(|s| (*s).to_owned()).collect(),
        ..AgentCfg::default()
    })
}

fn outputs() -> Vec<OutputInfo> {
    vec![output("DP-1", 0.), output("DP-2", 1920.)]
}

fn windows() -> Vec<WindowInfo> {
    vec![
        window("1", "foot", "~/src", "DP-1"),
        window("2", "firefox", "Bank — Firefox", "DP-2"),
    ]
}

#[allow(clippy::needless_pass_by_value)]
fn check(scope: &Scope, target: AgentTarget) -> Result<(), String> {
    scope.check(&target, &outputs(), &windows())
}

fn geometry(text: &str) -> AgentTarget {
    AgentTarget::Geometry(Geometry::parse(text).unwrap())
}

#[test]
fn an_output_limit_refuses_every_target_that_reaches_another_output() {
    let s = scope(&["DP-1"], &[]);
    let dp = |n: &str| AgentTarget::Output(OutputName::new(n).unwrap());
    assert!(check(&s, dp("DP-1")).is_ok());
    let refused = check(&s, dp("DP-2")).unwrap_err();
    assert!(
        refused.contains("DP-2") && refused.contains("agent config set"),
        "{refused}"
    );
    assert!(check(&s, geometry("10,10 100x100")).is_ok());
    assert!(
        check(&s, geometry("1900,10 100x100")).is_err(),
        "straddles DP-2"
    );
    assert!(check(&s, geometry("9000,0 10x10")).is_err(), "on no output");
    assert!(check(&s, AgentTarget::Fullscreen).is_err());
    assert!(
        check(
            &s,
            AgentTarget::Window(WindowSelector::AppId("foot".into()))
        )
        .is_ok()
    );
    assert!(check(&s, AgentTarget::Window(WindowSelector::Id("2".into()))).is_err());
}

#[test]
fn a_window_limit_allows_only_those_windows_and_every_match_is_checked() {
    let s = scope(&[], &["foot"]);
    assert!(check(&s, AgentTarget::Window(WindowSelector::Id("1".into()))).is_ok());
    assert!(
        check(
            &s,
            AgentTarget::Window(WindowSelector::AppId("firefox".into()))
        )
        .is_err()
    );
    assert!(
        check(&s, AgentTarget::Window(WindowSelector::Title("r".into()))).is_err(),
        "a title that also matches the bank window"
    );
    assert!(check(&s, AgentTarget::Fullscreen).is_err());
    assert!(check(&s, geometry("0,0 10x10")).is_err());
}

#[test]
fn no_limit_allows_everything_and_lists_show_only_what_is_allowed() {
    assert!(check(&Scope::default(), geometry("9000,0 10x10")).is_ok());
    let mut reply = AgentReply::ok(es_protocol::agent::ReplyKind::Windows);
    reply.windows = Some(
        ["foot", "firefox"]
            .iter()
            .map(|app| es_protocol::agent::WindowEntry {
                id: (*app).into(),
                app_id: Some((*app).into()),
                title: None,
                x: None,
                y: None,
                width: None,
                height: None,
                output: Some("DP-1".into()),
                focused: None,
            })
            .collect(),
    );
    scope(&[], &["foot"]).filter(&mut reply);
    let ids: Vec<_> = reply.windows.unwrap().into_iter().map(|w| w.id).collect();
    assert_eq!(ids, ["foot"]);
}
