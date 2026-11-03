use super::capture::{Fail, fail_of};
use super::ocr::Recognizer;
use es_app::agent_redact::{frame_of, redact_secrets};
use es_app::errors::Error;
use es_app::pipeline::frame_rgba;
use es_app::toast::{IconName, Toast, show_toast};
use es_model::platform::Frame;
use es_protocol::agent::{ErrorCode, Redaction};
use gpui::App;
use std::cell::Cell;
use std::sync::Arc;

pub(super) enum Verdict {
    Off,
    Clean,
    Covered(Arc<Frame>, usize),
    TesseractMissing,
}

pub(super) struct Redactor {
    pub(super) recognizer: Recognizer,
    told: Cell<bool>,
}

impl Redactor {
    pub(super) fn new(recognizer: Recognizer) -> Self {
        Self {
            recognizer,
            told: Cell::new(false),
        }
    }

    /// What goes to the agent in place of the capture, if anything, and what to tell the agent.
    /// The user hears once per daemon session that nothing is being hidden.
    pub(super) fn settle(&self, verdict: Verdict, cx: &mut App) -> (Option<Arc<Frame>>, Redaction) {
        match verdict {
            Verdict::Off => (None, Redaction::skipped("off")),
            Verdict::Clean => (None, Redaction::applied(0)),
            Verdict::Covered(frame, n) => (Some(frame), Redaction::applied(count(n))),
            Verdict::TesseractMissing => {
                if !self.told.replace(true) {
                    show_toast(
                        Toast::new("toast-agent-redact-missing", IconName::TriangleAlert),
                        cx,
                    );
                }
                (None, Redaction::skipped("tesseract_missing"))
            }
        }
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Runs off the main thread: recognition takes seconds on a full screen.
pub(super) fn inspect(
    on: bool,
    frame: &Frame,
    languages: &[String],
    recognizer: &Recognizer,
) -> Result<Verdict, Fail> {
    if !on {
        return Ok(Verdict::Off);
    }
    let image = frame_rgba(frame).map_err(fail_of)?;
    match redact_secrets(&image, languages, &**recognizer) {
        Ok(found) if found.regions == 0 => Ok(Verdict::Clean),
        Ok(found) => Ok(Verdict::Covered(
            Arc::new(frame_of(frame, &found.image)),
            found.regions,
        )),
        Err(Error::TesseractMissing) => Ok(Verdict::TesseractMissing),
        Err(Error::Cancelled) => Err(Fail::new(ErrorCode::Cancelled, "cancelled")),
        Err(other) => Err(Fail::new(
            ErrorCode::Failed,
            format!("could not look for secrets to hide: {other}"),
        )),
    }
}

#[cfg(test)]
#[path = "agent_redact_tests.rs"]
mod tests;
