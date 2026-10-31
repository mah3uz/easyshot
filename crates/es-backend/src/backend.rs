use crate::probe::Probe;
use crate::traits::{
    BindHelper, Capture, Clipboard, PointerQuery, VirtualInput, WindowList, WindowRules,
};
use bitflags::bitflags;
use es_desktop::{Appearance, FilePicker, Notifier, Shortcuts, Tray};
use es_model::platform::CursorSource;
use es_record::Recorder;
use std::sync::Arc;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Caps: u64 {
        const LAYER_SHELL = 1;
        const CAPTURE_OUTPUT = 2;
        const CAPTURE_WINDOW = 4;
        const WINDOW_LIST = 8;
        const WINDOW_GEOMETRY = 16;
        const CLIPBOARD_NO_FOCUS = 32;
        const TRAY = 64;
        const NOTIFY_ACTIONS = 128;
        const PORTAL_SHORTCUTS = 256;
        const SHORTCUT_TRIGGERS = 512;
        const POINTER_GLOBAL = 1024;
        const VIRTUAL_INPUT = 2048;
        const RECORDER = 4096;
        const OCR = 8192;
        const WINDOW_RULES = 16384;
        const BIND_CHECK = 32768;
        const FILE_PICKER = 65536;
        const APPEARANCE = 131072;
        const CURSOR_STREAM = 262144;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    LayerShell,
}

/// Doctor and logs only.
#[derive(Debug, Clone, Default)]
pub struct CompositorInfo {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReportEntry {
    pub capability: &'static str,
    pub chosen: Option<&'static str>,
    pub rejected: Vec<(&'static str, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderReport {
    pub entries: Vec<ReportEntry>,
}

pub struct Backend {
    pub capture: Box<dyn Capture>,
    pub windows: Box<dyn WindowList>,
    pub pointer: Option<Box<dyn PointerQuery>>,
    pub rules: Option<Box<dyn WindowRules>>,
    pub binds: Option<Box<dyn BindHelper>>,
    pub clipboard: Box<dyn Clipboard>,
    pub shortcuts: Box<dyn Shortcuts>,
    pub tray: Option<Box<dyn Tray>>,
    pub notifier: Box<dyn Notifier>,
    pub files: Option<Box<dyn FilePicker>>,
    pub appearance: Option<Box<dyn Appearance>>,
    /// Best provider first.
    pub recorders: Vec<Box<dyn Recorder>>,
    pub cursor: Option<Arc<dyn CursorSource>>,
    pub virtual_input: Option<Box<dyn VirtualInput>>,
    pub overlay: OverlayKind,
    /// Derived from the chosen providers and `Probe.tools`, never set by hand.
    pub caps: Caps,
    pub compositor: CompositorInfo,
    pub report: ProviderReport,
}

impl Backend {
    pub fn detect(_probe: &Probe) -> Backend {
        todo!()
    }
}
