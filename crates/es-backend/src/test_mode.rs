use crate::backend::Backend;
use crate::desktop::UserBinds;
use crate::error::{Error, Result};
use crate::probe::TestConfig;
use crate::traits::{Capture, Clipboard, PointerQuery, WindowList};
use es_desktop::{Notification, NotificationId, Notifier, RecordingTrayState, Tray, TrayModel};
use es_geom::Point;
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, WindowEvent, WindowId,
    WindowInfo,
};
use es_protocol::Route;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const EVENT_BUFFER: usize = 256;

/// The app forwards `Test.SetPointer` here.
#[derive(Clone, Default)]
pub struct TestHooks {
    pointer: Arc<Mutex<Option<Point>>>,
}

impl TestHooks {
    pub fn set_pointer(&self, at: Point) {
        *self.pointer.lock().unwrap_or_else(|e| e.into_inner()) = Some(at);
    }

    fn pointer(&self) -> Option<Point> {
        *self.pointer.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Never exposes the user's own outputs, windows, pointer, clipboard or input.
pub fn isolate(b: Backend, cfg: &TestConfig) -> Backend {
    let hooks = TestHooks::default();
    let windows: Arc<dyn WindowList> = Arc::new(TestWindows::new(b.windows, cfg.output.clone()));
    Backend {
        capture: Box::new(TestCapture::new(
            b.capture,
            cfg.output.clone(),
            windows.clone(),
        )),
        windows: Box::new(windows),
        pointer: Some(Box::new(TestPointer {
            hooks: hooks.clone(),
            output: cfg.output.clone(),
        })),
        clipboard: Box::new(ClipboardSink::new(cfg.dir.join("clipboard"))),
        shortcuts: Box::new(UserBinds::new()),
        tray: b.tray.map(|_| Box::new(NoopTray::new()) as Box<dyn Tray>),
        notifier: Box::new(TestNotifier::new()),
        virtual_input: b.virtual_input.filter(|_| cfg.virtual_input),
        test: Some(hooks),
        ..b
    }
}

fn forward<E: Send + 'static>(
    source: async_channel::Receiver<E>,
    mut map: impl FnMut(E) -> Option<E> + Send + 'static,
) -> async_channel::Receiver<E> {
    let (tx, rx) = async_channel::bounded(EVENT_BUFFER);
    let spawned = std::thread::Builder::new()
        .name("test-mode-filter".into())
        .spawn(move || {
            while let Ok(e) = source.recv_blocking() {
                if let Some(e) = map(e)
                    && tx.send_blocking(e).is_err()
                {
                    break;
                }
            }
        });
    if let Err(e) = spawned {
        tracing::error!("test mode: cannot start the event filter: {e}");
    }
    rx
}

pub struct TestWindows {
    inner: Box<dyn WindowList>,
    output: OutputId,
    events: async_channel::Receiver<WindowEvent>,
}

impl TestWindows {
    pub fn new(inner: Box<dyn WindowList>, output: OutputId) -> Self {
        let mut shown: HashSet<WindowId> = inner
            .snapshot()
            .into_iter()
            .filter(|w| on_output(w, &output))
            .map(|w| w.id)
            .collect();
        let out = output.clone();
        let events = forward(inner.events(), move |e| match e {
            WindowEvent::Opened(w) | WindowEvent::Changed(w) if on_output(&w, &out) => {
                let opened = shown.insert(w.id.clone());
                Some(if opened {
                    WindowEvent::Opened(w)
                } else {
                    WindowEvent::Changed(w)
                })
            }
            WindowEvent::Opened(w) | WindowEvent::Changed(w) => {
                shown.remove(&w.id).then_some(WindowEvent::Closed(w.id))
            }
            WindowEvent::Closed(id) => shown.remove(&id).then_some(WindowEvent::Closed(id)),
            WindowEvent::Focused(id) => shown.contains(&id).then_some(WindowEvent::Focused(id)),
        });
        TestWindows {
            inner,
            output,
            events,
        }
    }
}

// A window that reports no output may be the user's, so it stays hidden.
fn on_output(w: &WindowInfo, output: &OutputId) -> bool {
    w.output.as_ref() == Some(output)
}

impl WindowList for TestWindows {
    fn snapshot(&self) -> Vec<WindowInfo> {
        let mut windows = self.inner.snapshot();
        windows.retain(|w| on_output(w, &self.output));
        windows
    }
    fn has_geometry(&self) -> bool {
        self.inner.has_geometry()
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.events.clone()
    }
}

pub struct TestCapture {
    inner: Box<dyn Capture>,
    output: OutputId,
    windows: Arc<dyn WindowList>,
    events: async_channel::Receiver<OutputEvent>,
}

impl TestCapture {
    pub fn new(inner: Box<dyn Capture>, output: OutputId, windows: Arc<dyn WindowList>) -> Self {
        let out = output.clone();
        let events = forward(inner.output_events(), move |e| {
            let id = match &e {
                OutputEvent::Added(o) | OutputEvent::Changed(o) => &o.id,
                OutputEvent::Removed(id) => id,
            };
            (*id == out).then_some(e)
        });
        TestCapture {
            inner,
            output,
            windows,
            events,
        }
    }
}

impl Capture for TestCapture {
    fn outputs(&self) -> Vec<OutputInfo> {
        let mut outputs = self.inner.outputs();
        outputs.retain(|o| o.id == self.output);
        outputs
    }
    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        self.events.clone()
    }
    fn capture_outputs(&self, ids: &[OutputId], o: CaptureOpts) -> Result<Vec<Frame>> {
        if let Some(other) = ids.iter().find(|id| **id != self.output) {
            return Err(Error::NotFound(other.0.clone()));
        }
        self.inner.capture_outputs(ids, o)
    }
    fn capture_window(&self, w: &WindowId, o: CaptureOpts) -> Result<Frame> {
        if !self.windows.snapshot().iter().any(|i| i.id == *w) {
            return Err(Error::NotFound(w.0.clone()));
        }
        self.inner.capture_window(w, o)
    }
}

pub struct TestPointer {
    pub hooks: TestHooks,
    pub output: OutputId,
}

impl PointerQuery for TestPointer {
    fn pointer_global(&self) -> Option<Point> {
        self.hooks.pointer()
    }
    fn output_under_pointer(&self) -> Option<OutputId> {
        self.hooks.pointer().map(|_| self.output.clone())
    }
    fn focused_output(&self) -> Option<OutputId> {
        Some(self.output.clone())
    }
}

pub struct ClipboardSink {
    dir: PathBuf,
    next: AtomicU64,
}

impl ClipboardSink {
    pub fn new(dir: PathBuf) -> Self {
        ClipboardSink {
            dir,
            next: AtomicU64::new(1),
        }
    }
}

fn uri_lines(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| format!("file://{}\r\n", p.display()))
        .collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let io = |source| Error::Io {
        path: path.to_owned(),
        source,
    };
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)
}

impl Clipboard for ClipboardSink {
    fn set(&self, items: Vec<ClipItem>) -> Result<()> {
        std::fs::create_dir_all(&self.dir).map_err(|source| Error::Io {
            path: self.dir.clone(),
            source,
        })?;
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        for (i, item) in items.iter().enumerate() {
            let (ext, bytes): (&str, Vec<u8>) = match item {
                ClipItem::Png(png) => ("png", png.to_vec()),
                ClipItem::UriList(paths) => ("uri-list", uri_lines(paths).into_bytes()),
                ClipItem::GnomeCopiedFiles(paths) => (
                    "gnome-copied-files",
                    format!("copy\n{}", uri_lines(paths)).into_bytes(),
                ),
                ClipItem::Text(text) => ("txt", text.clone().into_bytes()),
            };
            write_atomic(&self.dir.join(format!("{n:06}-{i}.{ext}")), &bytes)?;
        }
        Ok(())
    }
    fn needs_focused_window(&self) -> bool {
        false
    }
}

struct NoopTray {
    _keep_open: async_channel::Sender<Route>,
    activations: async_channel::Receiver<Route>,
}

impl NoopTray {
    fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        NoopTray {
            _keep_open: tx,
            activations: rx,
        }
    }
}

impl Tray for NoopTray {
    fn set_menu(&self, _m: TrayModel) {}
    fn set_recording(&self, s: Option<RecordingTrayState>) {
        tracing::debug!(?s, "test mode: tray recording state");
    }
    fn set_agent(&self, _active: bool) {}
    fn activations(&self) -> async_channel::Receiver<Route> {
        self.activations.clone()
    }
}

struct TestNotifier {
    next: AtomicU32,
    _keep_open: async_channel::Sender<(NotificationId, String)>,
    actions: async_channel::Receiver<(NotificationId, String)>,
}

impl TestNotifier {
    fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        TestNotifier {
            next: AtomicU32::new(1),
            _keep_open: tx,
            actions: rx,
        }
    }
}

impl Notifier for TestNotifier {
    fn notify(&self, n: Notification) -> es_desktop::Result<NotificationId> {
        tracing::info!(title = %n.title, "test mode: notification not shown");
        Ok(NotificationId(self.next.fetch_add(1, Ordering::Relaxed)))
    }
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        self.actions.clone()
    }
    fn supports_actions(&self) -> bool {
        false
    }
}
