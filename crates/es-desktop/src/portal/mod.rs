mod file_chooser;
mod settings;
mod shortcuts;

pub use file_chooser::PortalFilePicker;
pub use settings::PortalAppearance;
pub use shortcuts::{PortalShortcuts, portal_trigger};

use crate::Error;
use ashpd::desktop::ResponseError;

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn portal_error(e: ashpd::Error) -> Error {
    if let ashpd::Error::Response(ResponseError::Cancelled) = e {
        return Error::Cancelled;
    }
    let text = e.to_string();
    if [
        "ServiceUnknown",
        "NameHasNoOwner",
        "not provided by any .service",
    ]
    .iter()
    .any(|m| text.contains(m))
    {
        return Error::Unsupported;
    }
    Error::Protocol(text)
}
