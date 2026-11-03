use es_geom::{Point, Rect};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// wl_output name, e.g. "DP-2"; GPUI display match: uuid v5(DNS, name).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutputId(pub String);

/// Compositor id (Hyprland address, sway con_id, niri id) or foreign-toplevel identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Normal,
    R90,
    R180,
    R270,
    Flipped,
    F90,
    F180,
    F270,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub id: OutputId,
    pub description: String,
    /// Global logical rectangle.
    pub logical: Rect,
    pub physical: (u32, u32),
    pub scale: f64,
    pub transform: Transform,
    pub refresh_mhz: u32,
    pub focused: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputEvent {
    Added(OutputInfo),
    Changed(OutputInfo),
    Removed(OutputId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: WindowId,
    pub app_id: String,
    pub title: String,
    pub pid: Option<u32>,
    /// None when the compositor gives no geometry.
    pub logical: Option<Rect>,
    pub output: Option<OutputId>,
    pub workspace: Option<String>,
    pub floating: bool,
    pub focus_rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    Opened(WindowInfo),
    Changed(WindowInfo),
    Closed(WindowId),
    Focused(WindowId),
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub output: Option<OutputId>,
    pub size_px: (u32, u32),
    pub logical: Rect,
    pub scale: f64,
    pub bgra: Arc<[u8]>,
    pub transform: Transform,
    pub captured_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureOpts {
    pub cursor: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClipItem {
    Png(Arc<[u8]>),
    UriList(Vec<PathBuf>),
    GnomeCopiedFiles(Vec<PathBuf>),
    Text(String),
}

pub trait CursorSource: Send + Sync {
    /// Global logical pointer position and when it was sampled.
    fn sample(&self) -> Option<(Point, Instant)>;
    /// For a source that must stream positions: a source that streams only while it lives.
    /// `None` when `sample` works without one.
    fn open(&self) -> Option<Arc<dyn CursorSource>> {
        None
    }
}

/// The compositor's keymap in XKB text form; `serial` changes whenever the keymap does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapText {
    pub serial: u64,
    pub text: Arc<str>,
}

pub trait KeymapSource: Send + Sync {
    /// `None` until the compositor has sent a keymap.
    fn keymap(&self) -> Option<KeymapText>;
}
