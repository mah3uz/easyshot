use crate::Error;
use crate::shm::{ShmBuffer, pick_format};
use crate::stream::{Counters, Shared, StreamEvent, StreamOptions, bump};
use crate::stream_retry::{AfterFailure, Retries};
use es_model::{PixelBuf, PixelFormat, VideoFrame};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::time::{ClockId, clock_gettime};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm_pool::WlShmPool};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, event_created_child,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1, FailureReason},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};

const RETRY_AFTER: Duration = Duration::from_millis(50);

fn pixel_format(f: Format) -> Option<PixelFormat> {
    match f {
        Format::Argb8888 => Some(PixelFormat::Argb8888),
        Format::Xrgb8888 => Some(PixelFormat::Xrgb8888),
        Format::Abgr8888 => Some(PixelFormat::Abgr8888),
        Format::Xbgr8888 => Some(PixelFormat::Xbgr8888),
        _ => None,
    }
}

fn monotonic_ns() -> u64 {
    let t = clock_gettime(ClockId::Monotonic);
    (t.tv_sec as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(t.tv_nsec as u64)
}

fn protocol(e: impl std::fmt::Display) -> Error {
    Error::Protocol(e.to_string())
}

/// A buffer from the pool large enough for `len` bytes, or a new one.
fn pooled(pool: &Receiver<Vec<u8>>, counters: &Counters, len: usize) -> Vec<u8> {
    while let Ok(v) = pool.try_recv() {
        if v.capacity() >= len {
            return v;
        }
    }
    bump(&counters.allocations);
    Vec::with_capacity(len)
}

struct Handle {
    handle: ExtForeignToplevelHandleV1,
    identifier: String,
    done: bool,
}

pub(crate) struct Session {
    conn: Connection,
    queue: EventQueue<Capture>,
    state: Capture,
}

struct Capture {
    shm: WlShm,
    list: Option<ExtForeignToplevelListV1>,
    handles: Vec<Handle>,
    chosen: bool,
    target: Option<ExtForeignToplevelHandleV1>,
    source: Option<ExtImageCaptureSourceV1>,
    session: Option<ExtImageCopyCaptureSessionV1>,
    pending_size: (u32, u32),
    pending_formats: Vec<Format>,
    constraints: Option<((u32, u32), Format)>,
    generation: u64,
    buffer: Option<ShmBuffer>,
    frame: Option<(ExtImageCopyCaptureFrameV1, u64)>,
    presented: Option<u64>,
    retries: Retries,
    interval: Duration,
    requested_at: Option<Instant>,
    wait_until: Option<Instant>,
    finished: bool,
    shared: Arc<Shared>,
    pool_tx: Sender<Vec<u8>>,
    pool_rx: Receiver<Vec<u8>>,
}

impl Session {
    pub fn open(
        identifier: &str,
        options: StreamOptions,
        shared: Arc<Shared>,
    ) -> Result<Self, Error> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<Capture>(&conn).map_err(protocol)?;
        let qh = queue.handle();
        let missing = |what| move |_| Error::Unsupported(what);
        let shm: WlShm = globals.bind(&qh, 1..=1, ()).map_err(missing("wl_shm"))?;
        let list: ExtForeignToplevelListV1 = globals
            .bind(&qh, 1..=1, ())
            .map_err(missing("ext-foreign-toplevel-list"))?;
        let sources: ExtForeignToplevelImageCaptureSourceManagerV1 =
            globals
                .bind(&qh, 1..=1, ())
                .map_err(missing("window capture"))?;
        let copy: ExtImageCopyCaptureManagerV1 = globals
            .bind(&qh, 1..=1, ())
            .map_err(missing("window capture"))?;
        let (pool_tx, pool_rx) = channel();
        let mut state = Capture {
            shm,
            list: Some(list),
            handles: Vec::new(),
            chosen: false,
            target: None,
            source: None,
            session: None,
            pending_size: (0, 0),
            pending_formats: Vec::new(),
            constraints: None,
            generation: 0,
            buffer: None,
            frame: None,
            presented: None,
            retries: Retries::default(),
            interval: match options.fps {
                0 => Duration::ZERO,
                fps => Duration::from_secs(1) / fps,
            },
            requested_at: None,
            wait_until: None,
            finished: false,
            shared,
            pool_tx,
            pool_rx,
        };
        for _ in 0..2 {
            queue.roundtrip(&mut state).map_err(protocol)?;
        }
        let i = state
            .handles
            .iter()
            .position(|h| h.done && h.identifier == identifier)
            .ok_or_else(|| Error::UnknownToplevel(identifier.to_owned()))?;
        let target = state.handles.swap_remove(i).handle;
        for h in state.handles.drain(..) {
            h.handle.destroy();
        }
        state.chosen = true;
        if let Some(list) = &state.list {
            list.stop();
        }
        let flags = if options.paint_cursor {
            Options::PaintCursors
        } else {
            Options::empty()
        };
        let source = sources.create_source(&target, &qh, ());
        state.session = Some(copy.create_session(&source, flags, &qh, ()));
        state.source = Some(source);
        state.target = Some(target);
        sources.destroy();
        copy.destroy();
        Ok(Session { conn, queue, state })
    }

    pub fn run(mut self, wake: &UnixStream) {
        if let Err(e) = self.event_loop(wake) {
            self.state.end(StreamEvent::Failed(e));
        }
        self.state.release();
        let _ = self.conn.flush();
    }

    fn event_loop(&mut self, wake: &UnixStream) -> Result<(), Error> {
        let qh = self.queue.handle();
        loop {
            self.queue
                .dispatch_pending(&mut self.state)
                .map_err(protocol)?;
            if self.state.finished {
                return Ok(());
            }
            if self.state.wait_until.is_some_and(|t| t <= Instant::now()) {
                self.state.request(&qh);
            }
            match self.conn.flush() {
                Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                r => r.map_err(protocol)?,
            }
            let Some(guard) = self.queue.prepare_read() else {
                continue;
            };
            let timeout = self.state.wait_until.map(|t| {
                let d = t.saturating_duration_since(Instant::now());
                Timespec {
                    tv_sec: d.as_secs() as i64,
                    tv_nsec: d.subsec_nanos().into(),
                }
            });
            let (wl_ready, stopped) = {
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
            // The owner only ever closes its end, so any wake-up means stop.
            if !stopped.is_empty() {
                return Ok(());
            }
            if !wl_ready.is_empty() {
                match guard.read() {
                    Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                    r => {
                        r.map_err(protocol)?;
                    }
                }
            }
        }
    }
}

impl Capture {
    fn end(&mut self, event: StreamEvent) {
        if !self.finished {
            self.finished = true;
            self.shared.post_end(event);
        }
    }

    fn release(&mut self) {
        if let Some((frame, _)) = self.frame.take() {
            frame.destroy();
        }
        self.buffer = None;
        if let Some(session) = self.session.take() {
            session.destroy();
        }
        if let Some(source) = self.source.take() {
            source.destroy();
        }
        if let Some(target) = self.target.take() {
            target.destroy();
        }
        for h in self.handles.drain(..) {
            h.handle.destroy();
        }
        if let Some(list) = self.list.take() {
            list.destroy();
        }
    }

    /// Asks for the next frame, or schedules the ask when the last one was too recent.
    fn request(&mut self, qh: &QueueHandle<Capture>) {
        if self.finished || self.frame.is_some() {
            return;
        }
        let (Some(session), Some((size, format))) = (&self.session, self.constraints) else {
            return;
        };
        let now = Instant::now();
        if let Some(due) = self.requested_at.map(|t| t + self.interval)
            && due > now
        {
            self.wait_until = Some(due);
            return;
        }
        self.wait_until = None;
        let fresh = !self
            .buffer
            .as_ref()
            .is_some_and(|b| (b.width, b.height) == size && b.format == format);
        if fresh {
            self.buffer = None;
            match ShmBuffer::new(&self.shm, qh, size, size.0.saturating_mul(4), format) {
                Ok(b) => self.buffer = Some(b),
                Err(e) => return self.end(StreamEvent::Failed(e)),
            }
            bump(&self.shared.counters.shm_buffers);
        }
        let Some(buffer) = &self.buffer else { return };
        let frame = session.create_frame(qh, ());
        frame.attach_buffer(&buffer.buffer);
        if fresh {
            // ShmBuffer::new bounds both sides well below i32::MAX.
            frame.damage_buffer(0, 0, size.0 as i32, size.1 as i32);
        }
        frame.capture();
        bump(&self.shared.counters.requests);
        self.requested_at = Some(now);
        self.presented = None;
        self.frame = Some((frame, self.generation));
    }

    fn ready(&mut self, qh: &QueueHandle<Capture>) {
        let Some((frame, _)) = self.frame.take() else {
            return;
        };
        frame.destroy();
        self.retries.ready();
        let Some(buf) = &self.buffer else { return };
        let Some(format) = pixel_format(buf.format) else {
            return self.end(StreamEvent::Failed(Error::Unsupported("shm format")));
        };
        let bytes = buf.bytes();
        let mut data = pooled(&self.pool_rx, &self.shared.counters, bytes.len());
        data.clear();
        data.extend_from_slice(bytes);
        let frame = VideoFrame {
            width: buf.width,
            height: buf.height,
            stride: buf.stride,
            format,
            data: PixelBuf::pooled(data, self.pool_tx.clone()),
            presented: self.presented.unwrap_or_else(monotonic_ns),
        };
        bump(&self.shared.counters.frames);
        self.shared.post_frame(frame);
        self.request(qh);
    }

    fn failed(&mut self, reason: WEnum<FailureReason>, qh: &QueueHandle<Capture>) {
        let Some((frame, generation)) = self.frame.take() else {
            return;
        };
        frame.destroy();
        bump(&self.shared.counters.failures);
        if reason == WEnum::Value(FailureReason::BufferConstraints) {
            self.buffer = None;
        }
        match self.retries.failed(reason, self.generation > generation) {
            AfterFailure::RequestNow => self.request(qh),
            // New constraints arriving before then ask for the next frame at once.
            AfterFailure::RetryLater => self.wait_until = Some(Instant::now() + RETRY_AFTER),
            AfterFailure::Stopped => self.end(StreamEvent::Stopped),
            AfterFailure::GiveUp => self.end(StreamEvent::Failed(Error::CaptureFailed(format!(
                "{reason:?}"
            )))),
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for Capture {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: ext_image_copy_capture_session_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_session_v1::Event;
        match event {
            Event::BufferSize { width, height } => state.pending_size = (width, height),
            Event::ShmFormat {
                format: WEnum::Value(f),
            } => state.pending_formats.push(f),
            Event::Done => {
                let formats = std::mem::take(&mut state.pending_formats);
                let Some(format) = pick_format(&formats, false) else {
                    return state.end(StreamEvent::Failed(Error::Unsupported("shm format")));
                };
                let constraints = Some((state.pending_size, format));
                state.retries.constraints(state.constraints != constraints);
                state.constraints = constraints;
                state.generation += 1;
                state.request(qh);
            }
            Event::Stopped => state.end(StreamEvent::Stopped),
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for Capture {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: ext_image_copy_capture_frame_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use ext_image_copy_capture_frame_v1::Event;
        match event {
            Event::PresentationTime {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
            } => {
                let secs = (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo);
                state.presented = Some(
                    secs.saturating_mul(1_000_000_000)
                        .saturating_add(u64::from(tv_nsec)),
                );
            }
            Event::Ready => state.ready(qh),
            Event::Failed { reason } => state.failed(reason, qh),
            _ => {}
        }
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for Capture {
    fn event(
        state: &mut Self,
        list: &ExtForeignToplevelListV1,
        event: ext_foreign_toplevel_list_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // Windows that open after ours was chosen arrive only until the stop takes effect.
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } if state.chosen => {
                toplevel.destroy();
            }
            ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } => {
                state.handles.push(Handle {
                    handle: toplevel,
                    identifier: String::new(),
                    done: false,
                });
            }
            ext_foreign_toplevel_list_v1::Event::Finished => {
                state.list = None;
                list.destroy();
            }
            _ => {}
        }
    }

    event_created_child!(Capture, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for Capture {
    fn event(
        state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: ext_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(h) = state.handles.iter_mut().find(|h| h.handle == *handle) else {
            return;
        };
        match event {
            ext_foreign_toplevel_handle_v1::Event::Identifier { identifier } => {
                h.identifier = identifier;
            }
            ext_foreign_toplevel_handle_v1::Event::Done => h.done = true,
            _ => {}
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for Capture {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

macro_rules! no_events {
    ($($t:ty),*) => {$(
        impl Dispatch<$t, ()> for Capture {
            fn event(_: &mut Self, _: &$t, _: <$t as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}

no_events!(
    WlShm,
    WlShmPool,
    WlBuffer,
    ExtImageCopyCaptureManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1
);
