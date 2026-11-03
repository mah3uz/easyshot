use es_app::toast::{IconName, Toast, show_toast};
use es_backend::Backend;
use es_model::platform::OutputInfo;
use es_model::recording::InputAccess;
use es_overlays::record::Recorded;
use es_record::Clock;
use es_record::input::{EventSource, InputCollector, Sources};
use gpui::App;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Opens the keyboards and mice for one recording, or says why it cannot.
pub type InputFor = Rc<dyn Fn() -> Result<Box<dyn EventSource>, InputAccess>>;

pub struct Inputs {
    pub open: InputFor,
    pub sources: Sources,
}

/// Test mode never reads real devices and has no login session to follow.
pub fn for_system(clock: Arc<dyn Clock>, backend: &Backend) -> Inputs {
    #[cfg(feature = "test-hooks")]
    let test = crate::daemon::test_mode_requested();
    #[cfg(not(feature = "test-hooks"))]
    let test = false;
    let session_open = Arc::new(AtomicBool::new(test));
    if !test {
        es_desktop::watch_login_session(session_open.clone());
    }
    Inputs {
        open: Rc::new(move || {
            #[cfg(feature = "test-hooks")]
            if test {
                return Ok(Box::new(super::fake::NoEvents));
            }
            es_record::input::open_system(clock.clone())
        }),
        sources: Sources {
            cursor: backend.cursor.clone(),
            keymap: backend.keymap.clone(),
            session_open,
        },
    }
}

/// Reads clicks and keys for `recorded`; without access the recording goes on without them.
pub fn start(
    inputs: &Inputs,
    recorded: &Recorded,
    outputs: &[OutputInfo],
    cx: &mut App,
) -> Option<InputCollector> {
    let events = match (inputs.open)() {
        Ok(events) => events,
        Err(access) => {
            tracing::warn!(?access, "recording without clicks and keys");
            let title = match access {
                InputAccess::NotBuilt => "toast-recording-input-not-built",
                InputAccess::GroupPending => "toast-recording-input-pending",
                _ => "toast-recording-no-input",
            };
            show_toast(Toast::new(title, IconName::Info), cx);
            return None;
        }
    };
    let rect = match recorded {
        Recorded::Area(r) => Some(*r),
        Recorded::Display(id) => outputs.iter().find(|o| o.id == *id).map(|o| o.logical),
    };
    let mut sources = inputs.sources.clone();
    sources.cursor = rect.and(sources.cursor);
    Some(InputCollector::start(
        rect.unwrap_or_default(),
        sources,
        events,
    ))
}

/// Stops reading at once; the thread is joined off the UI thread and what it read is dropped.
pub fn halt(collector: Option<InputCollector>, cx: &App) {
    if let Some(c) = collector {
        c.halt();
        cx.background_executor()
            .spawn(async move { drop(c) })
            .detach();
    }
}
