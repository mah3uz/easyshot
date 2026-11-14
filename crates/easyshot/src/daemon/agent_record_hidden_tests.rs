use super::*;

// The user is working: an agent's recording of a window must never switch their workspace.

fn away(caps: Caps) -> Sources {
    let mut windows = desk();
    windows.push(WindowInfo {
        logical: None,
        ..window("3", "term", "DP-1")
    });
    Sources {
        caps,
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

fn counted(asked: &Rc<Cell<u32>>) -> crate::daemon::recording::FocusWindow {
    let asked = asked.clone();
    Rc::new(move |w, _| {
        asked.set(asked.get() + 1);
        Task::ready(Ok(w.clone()))
    })
}

fn window_recorder() -> Option<FakeRecorder> {
    Some(FakeRecorder {
        window: true,
        ..FakeRecorder::default()
    })
}

fn term() -> AgentTarget {
    AgentTarget::Window(WindowSelector::Id("3".into()))
}

#[gpui::test]
async fn a_window_no_screen_shows_is_recorded_alone_without_focus(cx: &mut TestAppContext) {
    let asked = Rc::new(Cell::new(0));
    let rig = rig_with(
        agent_on(),
        away(Caps::all()),
        counted(&asked),
        window_recorder(),
        cx,
    );
    let started = rig.start(term(), 60, cx);
    assert_eq!(started["ok"], true, "{started}");
    assert!(rig.recording());
    assert_eq!(asked.get(), 0, "never brought forward");
}

#[gpui::test]
async fn without_window_recording_a_window_no_screen_shows_is_unsupported(cx: &mut TestAppContext) {
    let asked = Rc::new(Cell::new(0));
    let caps = Caps::all() - Caps::RECORD_WINDOW;
    let rig = rig_with(agent_on(), away(caps), counted(&asked), None, cx);
    let failed = rig.start(term(), 60, cx);
    assert_eq!(code(&failed), "unsupported", "{failed}");
    assert!(
        failed["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("only while it is on screen")
    );
    assert!(!rig.recording());
    assert_eq!(asked.get(), 0);
}

#[gpui::test]
async fn a_window_no_screen_shows_is_never_handed_to_the_screen_recorder(cx: &mut TestAppContext) {
    let asked = Rc::new(Cell::new(0));
    let rig = rig_with(agent_on(), away(Caps::all()), counted(&asked), None, cx);
    let failed = rig.start(term(), 60, cx);
    assert_eq!(code(&failed), "unsupported", "{failed}");
    assert!(!rig.recording(), "its output shows some other window");
    assert_eq!(asked.get(), 0);
}

#[gpui::test]
async fn a_window_no_screen_shows_outside_the_scope_is_refused(cx: &mut TestAppContext) {
    let asked = Rc::new(Cell::new(0));
    let scoped = AgentCfg {
        allowed_outputs: vec!["DP-2".into()],
        ..agent_on()
    };
    let rig = rig_with(
        scoped,
        away(Caps::all()),
        counted(&asked),
        window_recorder(),
        cx,
    );
    let failed = rig.start(term(), 60, cx);
    assert_eq!(code(&failed), "access_denied", "{failed}");
    assert!(!rig.recording());
    assert_eq!(asked.get(), 0);
}
