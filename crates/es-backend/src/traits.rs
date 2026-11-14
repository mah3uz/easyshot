use crate::error::Result;
use es_geom::{Point, Size};
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, WindowEvent, WindowId,
    WindowInfo,
};
use es_protocol::Route;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

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
    /// Every open window on any workspace; a window the screen does not show has no `logical`.
    fn all(&self) -> Vec<WindowInfo> {
        self.snapshot()
    }
    fn can_focus(&self) -> bool {
        false
    }
    /// The window's logical size, also while no screen shows it.
    fn size(&self, _w: &WindowId) -> Option<Size> {
        None
    }
    /// Gives the window focus, which also brings its workspace to an output.
    fn focus(&self, _w: &WindowId) -> Result<()> {
        Err(crate::Error::Unsupported)
    }
}

pub trait PointerQuery: Send + Sync {
    fn pointer_global(&self) -> Option<Point>;
    fn output_under_pointer(&self) -> Option<OutputId>;
    fn focused_output(&self) -> Option<OutputId>;
}

pub trait BindHelper: Send + Sync {
    fn syntax(&self) -> BindSyntax;
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String;
    fn existing_binds(&self) -> Result<Vec<(Trigger, String)>>;
    /// Name of easyshot's binds file inside its config directory.
    fn file_name(&self) -> &'static str;
    /// The whole binds file for these binds; an empty list gives a valid, empty file.
    fn file_text(&self, binds: &[(Trigger, Route)]) -> String;
    /// The binds a file written by `file_text` holds, as trigger and command.
    fn file_binds(&self, _text: &str) -> Vec<(Trigger, String)> {
        Vec::new()
    }
    /// The one line the user adds to the compositor config; `dir` is easyshot's config directory as the user writes it.
    fn include_line(&self, dir: &str) -> String;
    /// Whether the compositor config, or a file it includes directly, includes the binds file in `dir`.
    fn is_included(&self, dir: &Path) -> Result<bool>;
    /// True when the compositor only reads a changed binds file after a reload.
    fn needs_reload(&self) -> bool {
        false
    }
    fn reload(&self) -> Result<()> {
        Err(crate::Error::Unsupported)
    }
}

pub trait Clipboard: Send + Sync {
    fn set(&self, items: Vec<ClipItem>) -> Result<()>;
    fn needs_focused_window(&self) -> bool;
}

pub trait VirtualInput: Send + Sync {
    /// Wheel at `at`; then the pointer is left on `park` when given, so it and its hover effects stay out of what is captured.
    fn scroll(&self, at: Point, dy: f64, park: Option<Point>) -> Result<()>;
}

/// The compositor's own report of how it animates a layer surface away.
pub trait LayerFade: Send + Sync {
    /// How long a closed layer surface stays on screen, as the compositor is set up now.
    fn linger(&self) -> Result<Duration>;
}

/// The compositor's own report of its screen lock.
pub trait ScreenLock: Send + Sync {
    /// Whether a locker holds the screen now, then each change; closes when the compositor goes.
    fn watch(&self) -> async_channel::Receiver<bool>;
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
    fn all(&self) -> Vec<WindowInfo> {
        (**self).all()
    }
    fn can_focus(&self) -> bool {
        (**self).can_focus()
    }
    fn size(&self, w: &WindowId) -> Option<Size> {
        (**self).size(w)
    }
    fn focus(&self, w: &WindowId) -> Result<()> {
        (**self).focus(w)
    }
}
