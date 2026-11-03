mod backend;
mod doctor;
mod error;
mod include_scan;
mod probe;
pub mod registry;
#[cfg(feature = "test-hooks")]
pub mod test_mode;
mod traits;

pub mod desktop;
#[cfg(feature = "hyprland")]
pub mod hyprland;
#[cfg(feature = "niri")]
pub mod niri;
pub mod wayland;
#[cfg(feature = "wlroots")]
pub mod wlroots;

pub use backend::{Backend, Caps, CompositorInfo, OverlayKind, ProviderReport, ReportEntry};
pub use doctor::doctor_json;
pub use error::{BackendError, Error, Result};
#[cfg(feature = "test-hooks")]
pub use probe::TestConfig;
pub use probe::{DbusNames, Env, PortalIfaces, Probe, Provider, TOOL_PACKAGES, Tools, WlGlobals};
pub use traits::{
    BindHelper, BindSyntax, Capture, Clipboard, PointerQuery, Trigger, VirtualInput, WindowList,
};
