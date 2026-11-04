use super::*;

fn away() -> Sources {
    let mut windows = desk();
    windows.push(WindowInfo {
        logical: None,
        output: None,
        ..window("3", "term", "DP-1")
    });
    Sources {
        windows: Arc::new(move || windows.clone()),
        ..sources()
    }
}

fn agent_on() -> AgentCfg {
    AgentCfg {
        enabled: true,
        consent: es_model::Consent::AlwaysAllow,
        ..AgentCfg::default()
    }
}

#[gpui::test]
async fn a_window_on_another_workspace_is_focused_and_then_recorded(cx: &mut TestAppContext) {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = asked.clone();
    let focus: crate::daemon::recording::FocusWindow = Rc::new(move |w, _| {
        log.borrow_mut().push(w.id.0.clone());
        Task::ready(Ok(WindowInfo {
            logical: Some(Rect::new(0., 0., 200., 100.)),
            output: Some(OutputId("DP-1".into())),
            ..w.clone()
        }))
    });
    let rig = rig_with(agent_on(), away(), focus, cx);
    let started = rig.start(AgentTarget::Window(WindowSelector::Id("3".into())), 60, cx);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(asked.borrow().as_slice(), ["3"]);
    assert!(rig.recording());
}

#[gpui::test]
async fn a_window_that_cannot_be_reached_is_an_error_with_a_hint(cx: &mut TestAppContext) {
    let rig = rig_with(
        agent_on(),
        away(),
        Rc::new(|_, _| Task::ready(Err(es_app::errors::Error::WindowUnreachable))),
        cx,
    );
    let failed = rig.start(AgentTarget::Window(WindowSelector::Id("3".into())), 60, cx);
    assert_eq!(code(&failed), "failed", "{failed}");
    let hint = failed["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("switch"), "{hint}");
    assert!(!rig.recording());
}

#[gpui::test]
async fn a_window_already_on_screen_is_not_focused(cx: &mut TestAppContext) {
    let asked = Rc::new(Cell::new(0));
    let count = asked.clone();
    let focus: crate::daemon::recording::FocusWindow = Rc::new(move |w, _| {
        count.set(count.get() + 1);
        Task::ready(Ok(w.clone()))
    });
    let rig = rig_with(agent_on(), sources(), focus, cx);
    let started = rig.start(AgentTarget::Window(WindowSelector::Id("1".into())), 60, cx);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(asked.get(), 0);
}
