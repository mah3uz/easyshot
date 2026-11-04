use crate::error::{Error, Result};
use crate::probe::{Probe, TOOL_PACKAGES};
use crate::registry::{Registry, select, select_all};
use crate::traits::{
    BindHelper, Capture, Clipboard, LayerFade, PointerQuery, ScreenLock, VirtualInput, WindowList,
};
use crate::{desktop, wayland};
use bitflags::bitflags;
use es_desktop::{Appearance, FilePicker, Notifier, Shortcuts, Tray};
use es_model::platform::{
    CaptureOpts, ClipItem, CursorSource, Frame, KeymapSource, OutputEvent, OutputId, OutputInfo,
    WindowEvent, WindowId, WindowInfo,
};
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
    /// The value of `EASYSHOT_PROVIDER_<CAP>`, when set.
    pub forced: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderReport {
    pub entries: Vec<ReportEntry>,
    /// `(tool, package)` for each tool not on `PATH`.
    pub missing_tools: Vec<(&'static str, &'static str)>,
    pub errors: Vec<String>,
    pub wayland_globals: serde_json::Map<String, serde_json::Value>,
    pub portals: serde_json::Map<String, serde_json::Value>,
}

impl ProviderReport {
    pub fn chosen(&self, capability: &str) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|e| e.capability == capability)
            .and_then(|e| e.chosen)
    }
}

pub struct Backend {
    pub capture: Box<dyn Capture>,
    pub windows: Box<dyn WindowList>,
    pub pointer: Option<Box<dyn PointerQuery>>,
    pub binds: Option<Box<dyn BindHelper>>,
    pub fade: Option<Box<dyn LayerFade>>,
    pub clipboard: Box<dyn Clipboard>,
    pub shortcuts: Box<dyn Shortcuts>,
    pub tray: Option<Box<dyn Tray>>,
    pub notifier: Box<dyn Notifier>,
    pub files: Option<Box<dyn FilePicker>>,
    pub appearance: Option<Box<dyn Appearance>>,
    /// Best provider first.
    pub recorders: Vec<Box<dyn Recorder>>,
    pub cursor: Option<Arc<dyn CursorSource>>,
    pub keymap: Option<Arc<dyn KeymapSource>>,
    pub screen_lock: Option<Arc<dyn ScreenLock>>,
    pub virtual_input: Option<Box<dyn VirtualInput>>,
    pub overlay: OverlayKind,
    /// Derived from the chosen providers and `Probe.tools`, never set by hand.
    pub caps: Caps,
    pub compositor: CompositorInfo,
    pub report: ProviderReport,
    #[cfg(feature = "test-hooks")]
    pub test: Option<crate::test_mode::TestHooks>,
}

impl Backend {
    pub fn detect(probe: &Probe) -> Backend {
        Self::detect_with(probe, &Registry::default())
    }

    pub fn detect_with(probe: &Probe, reg: &Registry) -> Backend {
        let mut report = ProviderReport {
            missing_tools: TOOL_PACKAGES
                .iter()
                .copied()
                .filter(|(tool, _)| !probe.tools.has(tool))
                .collect(),
            errors: probe.errors.clone(),
            wayland_globals: probe
                .globals
                .all
                .iter()
                .map(|(name, version)| (name.clone(), serde_json::json!(version)))
                .collect(),
            portals: [
                ("FileChooser", probe.portals.file_chooser),
                ("Settings", probe.portals.settings),
                ("GlobalShortcuts", probe.portals.global_shortcuts),
                ("ScreenCast", probe.portals.screencast),
                ("Screenshot", probe.portals.screenshot),
            ]
            .into_iter()
            .map(|(name, on)| (name.to_owned(), serde_json::json!(on)))
            .collect(),
            ..ProviderReport::default()
        };
        let capture = select("capture", reg.capture, probe, &mut report);
        let windows: Arc<dyn WindowList> = match select("windows", reg.windows, probe, &mut report)
        {
            Some(w) => Arc::from(w),
            None => Arc::new(NoWindows::new()),
        };
        let capture: Box<dyn Capture> = match capture {
            Some(inner) if windows.has_geometry() => Box::new(wayland::WindowGeometry {
                inner,
                windows: windows.clone(),
            }),
            Some(inner) => inner,
            None => Box::new(NoCapture::new()),
        };
        let backend = Backend {
            capture,
            windows: Box::new(windows),
            pointer: select("pointer", reg.pointer, probe, &mut report),
            binds: select("binds", reg.binds, probe, &mut report),
            fade: select("fade", reg.fade, probe, &mut report),
            clipboard: select("clipboard", reg.clipboard, probe, &mut report)
                .unwrap_or_else(|| Box::new(NoClipboard)),
            virtual_input: select("virtual_input", reg.virtual_input, probe, &mut report),
            cursor: select("cursor", reg.cursor, probe, &mut report).map(Arc::from),
            keymap: wayland::keymap(probe),
            screen_lock: select("screen_lock", &[wayland::LOCK_NOTIFY], probe, &mut report)
                .map(Arc::from),
            shortcuts: select("shortcuts", reg.shortcuts, probe, &mut report)
                .unwrap_or_else(|| Box::new(desktop::UserBinds::new())),
            tray: select("tray", reg.tray, probe, &mut report),
            notifier: select("notifier", reg.notifier, probe, &mut report)
                .unwrap_or_else(|| Box::new(desktop::NoNotifier::new())),
            files: select("files", reg.files, probe, &mut report),
            appearance: select("appearance", reg.appearance, probe, &mut report),
            recorders: select_all("recorders", reg.recorders, probe, &mut report),
            overlay: OverlayKind::LayerShell,
            caps: Caps::empty(),
            compositor: CompositorInfo {
                name: probe.env.current_desktop().unwrap_or_default().to_owned(),
                version: None,
            },
            report,
            #[cfg(feature = "test-hooks")]
            test: None,
        };
        #[cfg(feature = "test-hooks")]
        let backend = match &probe.test {
            Some(cfg) => crate::test_mode::isolate(backend, cfg),
            None => backend,
        };
        let caps = derive_caps(&backend, probe);
        Backend { caps, ..backend }
    }
}

fn derive_caps(b: &Backend, probe: &Probe) -> Caps {
    let chosen = |cap: &str| b.report.chosen(cap);
    let mut caps = Caps::empty();
    caps.set(Caps::LAYER_SHELL, probe.globals.layer_shell.is_some());
    caps.set(Caps::CAPTURE_OUTPUT, chosen("capture").is_some());
    caps.set(
        Caps::CAPTURE_WINDOW,
        chosen("capture") == Some(wayland::EXT_IMAGE_COPY.name) && probe.globals.toplevel_capture(),
    );
    caps.set(Caps::WINDOW_LIST, chosen("windows").is_some());
    caps.set(Caps::WINDOW_GEOMETRY, b.windows.has_geometry());
    caps.set(
        Caps::CLIPBOARD_NO_FOCUS,
        chosen("clipboard").is_some() && !b.clipboard.needs_focused_window(),
    );
    caps.set(Caps::TRAY, b.tray.is_some());
    caps.set(Caps::NOTIFY_ACTIONS, b.notifier.supports_actions());
    caps.set(
        Caps::PORTAL_SHORTCUTS,
        chosen("shortcuts") == Some(desktop::PORTAL_GLOBAL_SHORTCUTS.name),
    );
    caps.set(Caps::SHORTCUT_TRIGGERS, chosen("binds").is_some());
    caps.set(Caps::POINTER_GLOBAL, b.pointer.is_some());
    caps.set(Caps::VIRTUAL_INPUT, b.virtual_input.is_some());
    caps.set(Caps::RECORDER, !b.recorders.is_empty());
    caps.set(Caps::OCR, probe.tools.tesseract);
    caps.set(Caps::BIND_CHECK, b.binds.is_some());
    caps.set(Caps::FILE_PICKER, b.files.is_some());
    caps.set(Caps::APPEARANCE, b.appearance.is_some());
    caps.set(Caps::CURSOR_STREAM, b.cursor.is_some());
    caps
}

struct NoCapture {
    _keep_open: async_channel::Sender<OutputEvent>,
    events: async_channel::Receiver<OutputEvent>,
}

impl NoCapture {
    fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        NoCapture {
            _keep_open: tx,
            events: rx,
        }
    }
}

impl Capture for NoCapture {
    fn outputs(&self) -> Vec<OutputInfo> {
        Vec::new()
    }
    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        self.events.clone()
    }
    fn capture_outputs(&self, _ids: &[OutputId], _o: CaptureOpts) -> Result<Vec<Frame>> {
        Err(Error::Unsupported)
    }
    fn capture_window(&self, _w: &WindowId, _o: CaptureOpts) -> Result<Frame> {
        Err(Error::Unsupported)
    }
}

struct NoWindows {
    _keep_open: async_channel::Sender<WindowEvent>,
    events: async_channel::Receiver<WindowEvent>,
}

impl NoWindows {
    fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        NoWindows {
            _keep_open: tx,
            events: rx,
        }
    }
}

impl WindowList for NoWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        Vec::new()
    }
    fn has_geometry(&self) -> bool {
        false
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.events.clone()
    }
}

struct NoClipboard;

impl Clipboard for NoClipboard {
    fn set(&self, _items: Vec<ClipItem>) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn needs_focused_window(&self) -> bool {
        true
    }
}
