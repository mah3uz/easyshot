mod backend;
mod error;
mod probe;
pub mod registry;
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
pub use error::{BackendError, Error, Result};
pub use probe::{DbusNames, Env, PortalIfaces, Probe, Provider, TestConfig, Tools, WlGlobals};
pub use traits::{
    BindHelper, BindSyntax, Capture, Clipboard, PointerQuery, RuleSet, Trigger, VirtualInput,
    WindowList, WindowRules,
};
