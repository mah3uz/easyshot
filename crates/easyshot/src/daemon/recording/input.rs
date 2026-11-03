use es_app::toast::{IconName, Toast, show_toast};
use es_model::platform::{CursorSource, OutputInfo};
use es_model::recording::InputAccess;
use es_overlays::record::Recorded;
use es_record::Clock;
use es_record::input::{EventSource, InputCollector};
use gpui::App;
use std::rc::Rc;
use std::sync::Arc;

/// Opens the keyboards and mice for one recording, or says why it cannot.
pub type InputFor = Rc<dyn Fn() -> Result<Box<dyn EventSource>, InputAccess>>;

/// Test mode never reads real devices.
pub fn for_system(clock: Arc<dyn Clock>) -> InputFor {
    Rc::new(move || {
        #[cfg(feature = "test-hooks")]
        if crate::daemon::test_mode_requested() {
            return Ok(Box::new(super::fake::NoEvents));
        }
        es_record::input::open_system(clock.clone())
    })
}

/// Reads clicks and keys for `recorded`; without access the recording goes on without them.
pub fn start(
    open: &InputFor,
    cursor: Option<&Arc<dyn CursorSource>>,
    recorded: &Recorded,
    outputs: &[OutputInfo],
    cx: &mut App,
) -> Option<InputCollector> {
    let events = match open() {
        Ok(events) => events,
        Err(access) => {
            tracing::warn!(?access, "recording without clicks and keys");
            let title = match access {
                InputAccess::NotBuilt => "toast-recording-input-not-built",
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
    let cursor = rect.and(cursor.cloned());
    Some(InputCollector::start(
        rect.unwrap_or_default(),
        cursor,
        events,
    ))
}
