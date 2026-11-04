use es_app::errors::{Error, Result};
use es_backend::{Backend, WindowList};
use es_model::platform::WindowInfo;
use es_overlays::record::RecordingTarget;
use gpui::{App, Task};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SHOWN_WITHIN: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(50);
const SWITCH_SETTLE: Duration = Duration::from_millis(250);

/// Brings a window that no screen shows onto one, and answers with where it now is.
pub type FocusWindow = Rc<dyn Fn(&WindowInfo, &mut App) -> Task<Result<WindowInfo>>>;

pub fn off_screen(target: &RecordingTarget) -> Option<&WindowInfo> {
    match target {
        RecordingTarget::Window(w) if w.logical.is_none() => Some(w),
        _ => None,
    }
}

pub fn from_backend(backend: Arc<Backend>) -> FocusWindow {
    Rc::new(move |w, cx| {
        let (backend, w) = (backend.clone(), w.clone());
        cx.background_executor().spawn(async move {
            bring_forward(&*backend.windows, &w, SHOWN_WITHIN, POLL, SWITCH_SETTLE)
        })
    })
}

/// Blocks while it waits; run it off the UI thread.
pub fn bring_forward(
    windows: &dyn WindowList,
    w: &WindowInfo,
    within: Duration,
    poll: Duration,
    settle: Duration,
) -> Result<WindowInfo> {
    if !windows.can_focus() {
        return Err(Error::WindowUnreachable);
    }
    windows.focus(&w.id).map_err(|e| {
        tracing::warn!(error = %e, window = %w.id.0, "window not focused");
        Error::WindowUnreachable
    })?;
    let deadline = Instant::now() + within;
    loop {
        let shown = windows
            .all()
            .into_iter()
            .find(|known| known.id == w.id && known.logical.is_some());
        if let Some(shown) = shown {
            std::thread::sleep(settle);
            return Ok(shown);
        }
        if Instant::now() >= deadline {
            return Err(Error::WindowUnreachable);
        }
        std::thread::sleep(poll);
    }
}
