use super::*;
use crate::daemon::recording::focus::bring_forward;
use es_backend::WindowList;
use es_model::platform::{WindowEvent, WindowId};
use std::sync::atomic::{AtomicUsize, Ordering};

fn window(id: &str, logical: Option<Rect>) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: "app".into(),
        title: "title".into(),
        pid: None,
        logical,
        output: Some(OutputId("A".into())),
        workspace: Some("2".into()),
        floating: false,
        focus_rank: None,
    }
}

fn on_other_workspace() -> RecordingRequest {
    RecordingRequest {
        target: RecordingTarget::Window(window("w2", None)),
        mic: None,
        system_audio: false,
        delay_secs: 0,
    }
}

#[gpui::test]
async fn a_window_on_another_workspace_is_focused_before_the_recorder_starts(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let started = cx.update(|cx| rig.recordings.with_request(on_other_workspace(), cx));
    started.await.unwrap();
    assert_eq!(*rig.focus.asked.borrow(), ["w2"]);
    assert_eq!(rig.starts.get(), 1);
    assert!(rig.recordings.is_recording());
}

#[gpui::test]
async fn a_window_already_on_screen_is_not_focused(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let mut request = on_other_workspace();
    request.target = RecordingTarget::Window(window("w1", Some(Rect::new(0.0, 0.0, 10.0, 10.0))));
    cx.update(|cx| rig.recordings.with_request(request, cx))
        .await
        .unwrap();
    assert!(rig.focus.asked.borrow().is_empty());
    assert!(rig.recordings.is_recording());
}

#[gpui::test]
async fn a_focus_that_fails_records_nothing_and_frees_the_recorder(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.focus.fails.set(true);
    let started = cx.update(|cx| rig.recordings.with_request(on_other_workspace(), cx));
    assert!(matches!(started.await, Err(Error::WindowUnreachable)));
    assert_eq!(rig.starts.get(), 0);
    assert!(!rig.recordings.is_busy());
}

struct Scripted {
    focusable: bool,
    appears_after: usize,
    polls: AtomicUsize,
    focused: Mutex<Vec<String>>,
}

impl WindowList for Scripted {
    fn snapshot(&self) -> Vec<WindowInfo> {
        Vec::new()
    }
    fn has_geometry(&self) -> bool {
        true
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        async_channel::unbounded().1
    }
    fn all(&self) -> Vec<WindowInfo> {
        let shown = self.polls.fetch_add(1, Ordering::SeqCst) + 1 > self.appears_after;
        vec![window("w2", shown.then(|| Rect::new(0.0, 0.0, 5.0, 5.0)))]
    }
    fn can_focus(&self) -> bool {
        self.focusable
    }
    fn focus(&self, w: &WindowId) -> es_backend::Result<()> {
        self.focused.lock().unwrap().push(w.0.clone());
        Ok(())
    }
}

fn scripted(focusable: bool, appears_after: usize) -> Scripted {
    Scripted {
        focusable,
        appears_after,
        polls: Default::default(),
        focused: Default::default(),
    }
}

const TICK: Duration = Duration::from_millis(1);

#[test]
fn bringing_forward_waits_until_the_window_is_on_an_output() {
    let windows = scripted(true, 3);
    let shown = bring_forward(
        &windows,
        &window("w2", None),
        Duration::from_secs(1),
        TICK,
        TICK,
    )
    .unwrap();
    assert!(shown.logical.is_some());
    assert_eq!(*windows.focused.lock().unwrap(), ["w2"]);
    assert_eq!(windows.polls.load(Ordering::SeqCst), 4);
}

#[test]
fn bringing_forward_gives_up_when_the_window_never_appears() {
    let windows = scripted(true, usize::MAX);
    let waited = bring_forward(
        &windows,
        &window("w2", None),
        Duration::from_millis(30),
        TICK,
        TICK,
    );
    assert!(matches!(waited, Err(Error::WindowUnreachable)));
}

#[test]
fn a_compositor_that_cannot_focus_is_never_asked() {
    let windows = scripted(false, 0);
    let result = bring_forward(
        &windows,
        &window("w2", None),
        Duration::from_secs(1),
        TICK,
        TICK,
    );
    assert!(matches!(result, Err(Error::WindowUnreachable)));
    assert!(windows.focused.lock().unwrap().is_empty());
}
