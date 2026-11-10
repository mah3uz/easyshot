use crate::error::{Error, Result};
use crate::portal::{self, Portal};
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
        const FOCUS_WINDOW = 524288;
        const RECORD_WINDOW = 1048576;
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
    pub tray: Option<Box<dyn Tray>>,
    pub notifier: Box<dyn Notifier>,
    /// Best provider first.
    pub recorders: Vec<Box<dyn Recorder>>,
    /// Records one window from its own frames; present only where the compositor offers them.
    pub window_recorder: Option<Arc<dyn Recorder>>,
    pub cursor: Option<Arc<dyn CursorSource>>,
    pub keymap: Option<Arc<dyn KeymapSource>>,
    pub screen_lock: Option<Arc<dyn ScreenLock>>,
    pub virtual_input: Option<Box<dyn VirtualInput>>,
    pub overlay: OverlayKind,
    /// Derived from the chosen providers and `Probe.tools`, never set by hand.
    pub(crate) caps: Caps,
    pub compositor: CompositorInfo,
    pub(crate) portal: Arc<Portal>,
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
            portals: portal::portals_report(&probe.portals),
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
        let pointer = select("pointer", reg.pointer, probe, &mut report);
        let binds = select("binds", reg.binds, probe, &mut report);
        let fade = select("fade", reg.fade, probe, &mut report);
        let clipboard = select("clipboard", reg.clipboard, probe, &mut report)
            .unwrap_or_else(|| Box::new(NoClipboard));
        let virtual_input = select("virtual_input", reg.virtual_input, probe, &mut report);
        let cursor = select("cursor", reg.cursor, probe, &mut report).map(Arc::from);
        let screen_lock =
            select("screen_lock", &[wayland::LOCK_NOTIFY], probe, &mut report).map(Arc::from);
        let shortcuts = portal::select_shortcuts(probe, reg, &mut report);
        let tray = select("tray", reg.tray, probe, &mut report);
        let notifier = select("notifier", reg.notifier, probe, &mut report)
            .unwrap_or_else(|| Box::new(desktop::NoNotifier::new()));
        let chosen = portal::Chosen {
            files: select("files", reg.files, probe, &mut report).map(Arc::from),
            appearance: select("appearance", reg.appearance, probe, &mut report).map(Arc::from),
            shortcuts,
        };
        let recorders = select_all("recorders", reg.recorders, probe, &mut report);
        let window_recorder = (report.chosen("capture") == Some(wayland::EXT_IMAGE_COPY.name)
            && probe.globals.toplevel_capture())
        .then(wayland::window_recorder)
        .flatten();
        let backend = Backend {
            capture,
            windows: Box::new(windows),
            pointer,
            binds,
            fade,
            clipboard,
            virtual_input,
            cursor,
            keymap: wayland::keymap(probe),
            screen_lock,
            tray,
            notifier,
            recorders,
            window_recorder,
            overlay: OverlayKind::LayerShell,
            caps: Caps::empty(),
            compositor: CompositorInfo {
                name: probe.env.current_desktop().unwrap_or_default().to_owned(),
                version: None,
            },
            portal: Arc::new(Portal::new(chosen, report, probe, reg)),
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

    /// Notification actions and the portal's providers are read as they are now.
    pub fn caps(&self) -> Caps {
        let mut caps = self.caps;
        caps.set(Caps::NOTIFY_ACTIONS, self.notifier.supports_actions());
        caps.set(Caps::FILE_PICKER, self.files().is_some());
        caps.set(Caps::APPEARANCE, self.appearance().is_some());
        caps.set(
            Caps::PORTAL_SHORTCUTS,
            self.portal.chosen("shortcuts") == Some(desktop::PORTAL_GLOBAL_SHORTCUTS.name),
        );
        caps
    }

    pub fn report(&self) -> ProviderReport {
        self.portal.report()
    }

    pub fn files(&self) -> Option<Arc<dyn FilePicker>> {
        self.portal.files()
    }

    pub fn appearance(&self) -> Option<Arc<dyn Appearance>> {
        self.portal.appearance()
    }

    pub fn shortcuts(&self) -> Arc<dyn Shortcuts> {
        self.portal.shortcuts()
    }

    /// Starts following the desktop portal, so its providers come and go with it.
    pub fn follow_portal(&self) {
        portal::follow(&self.portal);
    }

    /// Receives one message each time the portal's providers change; the daemon has one listener.
    pub fn portal_changes(&self) -> async_channel::Receiver<()> {
        self.portal.changes()
    }
}

fn derive_caps(b: &Backend, probe: &Probe) -> Caps {
    let report = b.report();
    let chosen = |cap: &str| report.chosen(cap);
    let mut caps = Caps::empty();
    caps.set(Caps::LAYER_SHELL, probe.globals.layer_shell.is_some());
    caps.set(Caps::CAPTURE_OUTPUT, chosen("capture").is_some());
    caps.set(
        Caps::CAPTURE_WINDOW,
        chosen("capture") == Some(wayland::EXT_IMAGE_COPY.name) && probe.globals.toplevel_capture(),
    );
    caps.set(Caps::WINDOW_LIST, chosen("windows").is_some());
    caps.set(Caps::WINDOW_GEOMETRY, b.windows.has_geometry());
    caps.set(Caps::FOCUS_WINDOW, b.windows.can_focus());
    caps.set(
        Caps::CLIPBOARD_NO_FOCUS,
        chosen("clipboard").is_some() && !b.clipboard.needs_focused_window(),
    );
    caps.set(Caps::TRAY, b.tray.is_some());
    caps.set(Caps::SHORTCUT_TRIGGERS, chosen("binds").is_some());
    caps.set(Caps::POINTER_GLOBAL, b.pointer.is_some());
    caps.set(Caps::VIRTUAL_INPUT, b.virtual_input.is_some());
    caps.set(Caps::RECORDER, !b.recorders.is_empty());
    caps.set(Caps::RECORD_WINDOW, b.window_recorder.is_some());
    caps.set(Caps::OCR, probe.tools.tesseract);
    caps.set(Caps::BIND_CHECK, b.binds.is_some());
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
