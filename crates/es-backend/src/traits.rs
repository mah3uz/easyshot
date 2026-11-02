use crate::error::Result;
use es_geom::Point;
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, WindowEvent, WindowId,
    WindowInfo,
};
use es_protocol::Route;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct RuleSet {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindSyntax {
    HyprLua,
    Hyprlang,
    Sway,
    NiriKdl,
}

/// Compositor-syntax key trigger, e.g. "SUPER+SHIFT+4".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger(pub String);

pub trait Capture: Send + Sync {
    fn outputs(&self) -> Vec<OutputInfo>;
    fn output_events(&self) -> async_channel::Receiver<OutputEvent>;
    fn capture_outputs(&self, ids: &[OutputId], o: CaptureOpts) -> Result<Vec<Frame>>;
    /// `Err(Error::Unsupported)` means the caller crops from the frozen frame.
    fn capture_window(&self, w: &WindowId, o: CaptureOpts) -> Result<Frame>;
}

pub trait WindowList: Send + Sync {
    fn snapshot(&self) -> Vec<WindowInfo>;
    fn has_geometry(&self) -> bool;
    fn events(&self) -> async_channel::Receiver<WindowEvent>;
}

pub trait PointerQuery: Send + Sync {
    fn pointer_global(&self) -> Option<Point>;
    fn output_under_pointer(&self) -> Option<OutputId>;
    fn focused_output(&self) -> Option<OutputId>;
}

pub trait WindowRules: Send + Sync {
    fn install(&self, rules: &RuleSet) -> Result<()>;
    fn remove(&self) -> Result<()>;
}

pub trait BindHelper: Send + Sync {
    fn syntax(&self) -> BindSyntax;
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String;
    fn existing_binds(&self) -> Result<Vec<(Trigger, String)>>;
}

pub trait Clipboard: Send + Sync {
    fn set(&self, items: Vec<ClipItem>) -> Result<()>;
    fn needs_focused_window(&self) -> bool;
}

pub trait VirtualInput: Send + Sync {
    fn scroll(&self, at: Point, dy: f64) -> Result<()>;
}

impl<T: WindowList + ?Sized> WindowList for Arc<T> {
    fn snapshot(&self) -> Vec<WindowInfo> {
        (**self).snapshot()
    }
    fn has_geometry(&self) -> bool {
        (**self).has_geometry()
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        (**self).events()
    }
}
