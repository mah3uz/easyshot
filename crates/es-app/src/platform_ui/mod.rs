mod clipboard;
mod coming_soon_surface;
pub mod layer;
mod pinned;
mod share_popover_surface;
mod surfaces;
mod toast_surface;

pub use clipboard::{BackendClipboardSink, with_file_items};
pub use coming_soon_surface::{ComingSoonHost, ComingSoonSurface};
pub use pinned::{PinnedOutput, PinnedOutputSource};
pub use share_popover_surface::{SharePopoverPanel, SharePopoverSurface};
pub use surfaces::{Gate, LayerShellSurfaces, LayerSurfaceHandle, OutputSource};
pub use toast_surface::{ToastHost, ToastSurface, card_icon};

use crate::errors::Result;
use es_geom::{Point, Rect, Rgba, Size};
use es_model::platform::{ClipItem, Frame, OutputId, WindowId};
use gpui::{AnyView, App, Task};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceRole {
    Overlay,
    Countdown,
    Toast,
    Deck,
    Pin,
    Bar,
    RecordControl,
    Highlight,
    Scroll,
    Consent,
    /// A panel centred over a scrim that covers its output; a press on the scrim or Esc closes it.
    Panel,
}

pub trait Surfaces {
    /// Test mode asserts `output == test output`.
    fn open(
        &self,
        role: SurfaceRole,
        output: &OutputId,
        size: Size,
        cx: &mut App,
    ) -> Result<Box<dyn SurfaceHandle>>;
}

pub trait SurfaceHandle {
    fn set_content(&self, view: AnyView, cx: &mut App);
    fn set_position(&self, global_logical: Point, cx: &mut App);
    fn resize(&self, size: Size, cx: &mut App);
    fn set_input_region(&self, rects: &[Rect], cx: &mut App);
    fn output(&self) -> OutputId;
    fn close(&self, cx: &mut App);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Region,
    Window,
    Text,
    Color,
    /// Starts by drawing the area to scroll; Space picks a window instead.
    Scroll,
    /// Starts by picking the window to scroll; Space draws a region instead.
    ScrollWindow,
}

#[derive(Debug, Clone)]
pub enum Selection {
    Region {
        output: OutputId,
        logical: Rect,
        frame: Arc<Frame>,
    },
    Window {
        id: WindowId,
        frame: Arc<Frame>,
    },
    Color(Rgba),
    Cancelled,
}

/// What a newer request does to a selection that is still being picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Switch {
    To(SelectMode),
    /// Ends the pick with the whole frozen output under the pointer.
    Whole,
    Close,
}

pub trait SelectionFlow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>>;

    /// Acts on the frames the running pick already froze; false once nothing is being picked.
    fn switch(&self, _to: Switch, _cx: &mut App) -> bool {
        false
    }

    /// The mode the running pick shows now, which the user may have changed.
    fn showing(&self) -> Option<SelectMode> {
        None
    }
}

pub trait ClipboardSink {
    fn copy(&self, items: Vec<ClipItem>, cx: &mut App) -> Task<Result<()>>;
}
