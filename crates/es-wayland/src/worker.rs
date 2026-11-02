use crate::capture::{Batch, CAPTURE_TIMEOUT, Job, Reply};
use crate::clipboard::{Clipboard, Offers, offers};
use crate::input::{CursorSession, Input};
use crate::output::OutputState;
use crate::toplevel::{Toplevel, ToplevelState, WlrToplevel};
use crate::{Error, WlGlobals};
use es_geom::Point;
use es_model::platform::{
    CaptureOpts, ClipItem, Frame, OutputEvent, OutputId, OutputInfo, WindowEvent, WindowInfo,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::{Dispatch, EventQueue, QueueHandle};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::ExtForeignToplevelListV1;
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;

const SETUP_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const WINDOW_EVENTS: usize = 1024;

pub(crate) enum Command {
    CaptureOutputs(
        Vec<OutputId>,
        CaptureOpts,
        Sender<Result<Vec<Frame>, Error>>,
    ),
    CaptureToplevel(String, CaptureOpts, Sender<Result<Frame, Error>>),
    Toplevels(Sender<Vec<Toplevel>>),
    SetClipboard(Offers, Sender<Result<(), Error>>),
    Scroll(Point, f64, Sender<Result<(), Error>>),
    CursorSession(Sender<Result<CursorSession, Error>>),
    #[cfg(test)]
    DisableExtCapture,
    #[cfg(test)]
    DisableExtDataControl,
    Shutdown,
}

pub(crate) struct State {
    pub globals: WlGlobals,
    advertised: Vec<(u32, String, u32)>,
    pub outputs: Vec<OutputState>,
    pub output_tx: async_channel::Sender<OutputEvent>,
    pub output_snapshot: Arc<Mutex<Vec<OutputInfo>>>,
    pub shm: Option<WlShm>,
    pub xdg_manager: Option<ZxdgOutputManagerV1>,
    pub ext_copy: Option<ExtImageCopyCaptureManagerV1>,
    pub ext_output_source: Option<ExtOutputImageCaptureSourceManagerV1>,
    pub ext_toplevel_source: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    pub wlr_screencopy: Option<ZwlrScreencopyManagerV1>,
    pub toplevel_list: Option<ExtForeignToplevelListV1>,
    pub toplevels: Vec<ToplevelState>,
    pub wlr_toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    pub wlr_toplevels: Vec<WlrToplevel>,
    pub focus_order: Vec<String>,
    pub window_tx: async_channel::Sender<WindowEvent>,
    pub window_snapshot: Arc<Mutex<Vec<WindowInfo>>>,
    pub seat: Option<WlSeat>,
    pub clipboard: Clipboard,
    pub input: Input,
    pub jobs: HashMap<u64, Job>,
    pub batches: HashMap<u64, Batch>,
    pub next_id: u64,
}

impl State {
    fn handle(&mut self, cmd: Command, qh: &QueueHandle<State>) -> bool {
        match cmd {
            Command::CaptureOutputs(ids, opts, tx) => {
                self.capture_outputs(&ids, opts, Reply::Outputs(tx), qh)
            }
            Command::CaptureToplevel(id, opts, tx) => {
                self.capture_toplevel(&id, opts, Reply::Toplevel(tx), qh)
            }
            Command::Toplevels(tx) => {
                let _ = tx.send(self.toplevel_list());
            }
            Command::SetClipboard(offers, tx) => {
                let _ = tx.send(self.set_clipboard(offers, qh));
            }
            Command::Scroll(at, dy, tx) => {
                let _ = tx.send(self.scroll(at, dy, qh));
            }
            Command::CursorSession(tx) => {
                let _ = tx.send(self.cursor_session(qh));
            }
            #[cfg(test)]
            Command::DisableExtCapture => self.ext_output_source = None,
            #[cfg(test)]
            Command::DisableExtDataControl => self.clipboard.disable_ext(),
            Command::Shutdown => return false,
        }
        true
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                match interface.as_str() {
                    "wl_output" => {
                        let wl: WlOutput = registry.bind(name, version.min(4), qh, name);
                        let mut o = OutputState::new(name, wl);
                        if let Some(m) = &state.xdg_manager {
                            o.attach_xdg(m, qh);
                        }
                        state.outputs.push(o);
                    }
                    "zxdg_output_manager_v1" => {
                        let m: ZxdgOutputManagerV1 = registry.bind(name, version.min(3), qh, ());
                        for o in &mut state.outputs {
                            o.attach_xdg(&m, qh);
                        }
                        state.xdg_manager = Some(m);
                    }
                    "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                    "ext_image_copy_capture_manager_v1" => {
                        state.ext_copy = Some(registry.bind(name, 1, qh, ()))
                    }
                    "ext_output_image_capture_source_manager_v1" => {
                        state.ext_output_source = Some(registry.bind(name, 1, qh, ()))
                    }
                    "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                        state.ext_toplevel_source = Some(registry.bind(name, 1, qh, ()))
                    }
                    "zwlr_screencopy_manager_v1" => {
                        state.wlr_screencopy = Some(registry.bind(name, version.min(3), qh, ()))
                    }
                    "ext_foreign_toplevel_list_v1" => {
                        state.toplevel_list = Some(registry.bind(name, 1, qh, ()))
                    }
                    "zwlr_foreign_toplevel_manager_v1" => {
                        state.wlr_toplevel_manager =
                            Some(registry.bind(name, version.min(3), qh, ()))
                    }
                    "wl_seat" if state.seat.is_none() => {
                        state.seat = Some(registry.bind(name, version.min(5), qh, ()))
                    }
                    "ext_data_control_manager_v1" => {
                        state.clipboard.ext = Some(registry.bind(name, 1, qh, ()))
                    }
                    "zwlr_data_control_manager_v1" => {
                        state.clipboard.wlr = Some(registry.bind(name, 1, qh, ()))
                    }
                    "zwlr_virtual_pointer_manager_v1" => {
                        state.input.virtual_manager =
                            Some(registry.bind(name, version.min(2), qh, ()))
                    }
                    _ => {}
                }
                state.advertised.push((name, interface, version));
                state.refresh_globals();
            }
            wl_registry::Event::GlobalRemove { name } => {
                state.remove_output(name);
                state.advertised.retain(|(n, _, _)| *n != name);
                state.refresh_globals();
            }
            _ => {}
        }
    }
}

impl State {
    fn refresh_globals(&mut self) {
        self.globals =
            WlGlobals::from_globals(self.advertised.iter().map(|(_, i, v)| (i.as_str(), *v)));
    }
}

pub struct Connection {
    commands: Sender<Command>,
    wake: UnixStream,
    globals: WlGlobals,
    outputs: Arc<Mutex<Vec<OutputInfo>>>,
    events: async_channel::Receiver<OutputEvent>,
    windows: Arc<Mutex<Vec<WindowInfo>>>,
    window_events: async_channel::Receiver<WindowEvent>,
    thread: Option<JoinHandle<()>>,
}

impl Connection {
    pub fn connect() -> Result<Self, Error> {
        let (commands, cmd_rx) = mpsc::channel();
        let (wake, wake_rx) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        wake_rx.set_nonblocking(true)?;
        let (output_tx, events) = async_channel::unbounded();
        let outputs = Arc::new(Mutex::new(Vec::new()));
        let (window_tx, window_events) = async_channel::bounded(WINDOW_EVENTS);
        let windows = Arc::new(Mutex::new(Vec::new()));
        let (setup_tx, setup_rx) = mpsc::channel();
        let published = Published {
            outputs: outputs.clone(),
            output_tx,
            windows: windows.clone(),
            window_tx,
        };
        let thread = std::thread::Builder::new()
            .name("es-wayland".into())
            .spawn(move || run(&setup_tx, &cmd_rx, &wake_rx, published))?;
        let globals = match setup_rx.recv_timeout(SETUP_TIMEOUT) {
            Ok(Ok(g)) => g,
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return Err(Error::Timeout),
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Error::Disconnected),
        };
        Ok(Connection {
            commands,
            wake,
            globals,
            outputs,
            events,
            windows,
            window_events,
            thread: Some(thread),
        })
    }

    pub fn globals(&self) -> &WlGlobals {
        &self.globals
    }

    pub fn outputs(&self) -> Vec<OutputInfo> {
        self.outputs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        self.events.clone()
    }

    pub fn capture_outputs(
        &self,
        ids: &[OutputId],
        opts: CaptureOpts,
    ) -> Result<Vec<Frame>, Error> {
        self.request(
            |tx| Command::CaptureOutputs(ids.to_vec(), opts, tx),
            CAPTURE_TIMEOUT,
        )?
    }

    pub fn toplevels(&self) -> Result<Vec<Toplevel>, Error> {
        self.request(Command::Toplevels, REQUEST_TIMEOUT)
    }

    pub fn windows(&self) -> Vec<WindowInfo> {
        self.windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn window_events(&self) -> async_channel::Receiver<WindowEvent> {
        self.window_events.clone()
    }

    pub fn set_clipboard(&self, items: &[ClipItem]) -> Result<(), Error> {
        let offers = offers(items)?;
        self.request(|tx| Command::SetClipboard(offers, tx), REQUEST_TIMEOUT)?
    }

    pub fn scroll(&self, at: Point, dy: f64) -> Result<(), Error> {
        self.request(|tx| Command::Scroll(at, dy, tx), REQUEST_TIMEOUT)?
    }

    pub fn cursor_session(&self) -> Result<CursorSession, Error> {
        self.request(Command::CursorSession, REQUEST_TIMEOUT)?
    }

    pub fn capture_toplevel(&self, identifier: &str, opts: CaptureOpts) -> Result<Frame, Error> {
        self.request(
            |tx| Command::CaptureToplevel(identifier.to_owned(), opts, tx),
            CAPTURE_TIMEOUT,
        )?
    }

    pub(crate) fn send(&self, cmd: Command) -> Result<(), Error> {
        self.commands.send(cmd).map_err(|_| Error::Disconnected)?;
        match (&self.wake).write(&[1]) {
            Err(e) if e.kind() != ErrorKind::WouldBlock => Err(e.into()),
            _ => Ok(()),
        }
    }

    fn request<T>(
        &self,
        make: impl FnOnce(Sender<T>) -> Command,
        timeout: Duration,
    ) -> Result<T, Error> {
        let (tx, rx) = mpsc::channel();
        self.send(make(tx))?;
        rx.recv_timeout(timeout + Duration::from_secs(1))
            .map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => Error::Timeout,
                mpsc::RecvTimeoutError::Disconnected => Error::Disconnected,
            })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Published {
    outputs: Arc<Mutex<Vec<OutputInfo>>>,
    output_tx: async_channel::Sender<OutputEvent>,
    windows: Arc<Mutex<Vec<WindowInfo>>>,
    window_tx: async_channel::Sender<WindowEvent>,
}

fn run(
    setup: &Sender<Result<WlGlobals, Error>>,
    commands: &Receiver<Command>,
    wake: &UnixStream,
    published: Published,
) {
    let (conn, mut queue, mut state) = match start(published) {
        Ok(v) => v,
        Err(e) => {
            let _ = setup.send(Err(e));
            return;
        }
    };
    if setup.send(Ok(state.globals.clone())).is_err() {
        return;
    }
    let qh = queue.handle();
    if let Err(e) = event_loop(&conn, &mut queue, &mut state, commands, wake, &qh) {
        tracing::error!(error = %e, "wayland worker stopped");
    }
}

type Started = (wayland_client::Connection, EventQueue<State>, State);

fn start(published: Published) -> Result<Started, Error> {
    let conn = wayland_client::Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = State {
        globals: WlGlobals::default(),
        advertised: Vec::new(),
        outputs: Vec::new(),
        output_tx: published.output_tx,
        output_snapshot: published.outputs,
        shm: None,
        xdg_manager: None,
        ext_copy: None,
        ext_output_source: None,
        ext_toplevel_source: None,
        wlr_screencopy: None,
        toplevel_list: None,
        toplevels: Vec::new(),
        wlr_toplevel_manager: None,
        wlr_toplevels: Vec::new(),
        focus_order: Vec::new(),
        window_tx: published.window_tx,
        window_snapshot: published.windows,
        seat: None,
        clipboard: Clipboard::default(),
        input: Input::new(),
        jobs: HashMap::new(),
        batches: HashMap::new(),
        next_id: 0,
    };
    for _ in 0..2 {
        queue.roundtrip(&mut state).map_err(protocol)?;
    }
    Ok((conn, queue, state))
}

fn event_loop(
    conn: &wayland_client::Connection,
    queue: &mut EventQueue<State>,
    state: &mut State,
    commands: &Receiver<Command>,
    wake: &UnixStream,
    qh: &QueueHandle<State>,
) -> Result<(), Error> {
    loop {
        queue.dispatch_pending(state).map_err(protocol)?;
        match conn.flush() {
            Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
            r => r.map_err(protocol)?,
        }
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let timeout = state
            .capture_deadline()
            .map(|d| d.saturating_duration_since(Instant::now()))
            .map(|d| Timespec {
                tv_sec: d.as_secs() as i64,
                tv_nsec: d.subsec_nanos().into(),
            });
        let (wl_ready, woken) = {
            let fd = guard.connection_fd();
            let mut fds = [
                PollFd::new(&fd, PollFlags::IN),
                PollFd::new(wake, PollFlags::IN),
            ];
            match poll(&mut fds, timeout.as_ref()) {
                Ok(_) => (fds[0].revents(), fds[1].revents()),
                Err(rustix::io::Errno::INTR) => (PollFlags::empty(), PollFlags::empty()),
                Err(e) => return Err(Error::Io(e.into())),
            }
        };
        if !wl_ready.is_empty() {
            match guard.read() {
                Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                r => {
                    r.map_err(protocol)?;
                }
            }
        } else {
            drop(guard);
        }
        if !woken.is_empty() {
            let mut buf = [0u8; 64];
            while matches!((&*wake).read(&mut buf), Ok(n) if n > 0) {}
        }
        loop {
            match commands.try_recv() {
                Ok(cmd) => {
                    if !state.handle(cmd, qh) {
                        return Ok(());
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        state.expire_captures(Instant::now());
    }
}

fn protocol(e: impl std::fmt::Display) -> Error {
    Error::Protocol(e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn connection_can_be_shared_across_threads() {
        fn shared<T: Send + Sync>() {}
        shared::<super::Connection>();
    }
}
