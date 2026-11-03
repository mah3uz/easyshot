mod capture;

pub use capture::{WindowGeometry, fit_window_frame, normalise_frame};

use crate::error::{BackendError, Result};
use crate::probe::{Probe, Provider};
use crate::traits::{Capture, Clipboard, ScreenLock, VirtualInput, WindowList};
use capture::WaylandCapture;
use es_geom::Point;
use es_model::platform::{
    ClipItem, CursorSource, KeymapSource, KeymapText, WindowEvent, WindowInfo,
};
use es_wayland::{Connection, CursorSession};
use std::sync::Arc;
use std::time::Instant;

fn connection(p: &Probe) -> std::result::Result<Arc<Connection>, BackendError> {
    p.wayland
        .clone()
        .ok_or_else(|| BackendError::Unavailable("no Wayland connection".into()))
}

pub const EXT_IMAGE_COPY: Provider<dyn Capture> = Provider {
    name: "ext-image-copy",
    probe: |p| p.globals.ext_output_capture().then_some(20),
    build: |p| {
        Ok(Box::new(WaylandCapture {
            conn: connection(p)?,
            windows: p.globals.toplevel_capture(),
        }))
    },
};

pub const WLR_SCREENCOPY: Provider<dyn Capture> = Provider {
    name: "wlr-screencopy",
    probe: |p| (p.globals.shm.is_some() && p.globals.wlr_screencopy.is_some()).then_some(10),
    build: |p| {
        Ok(Box::new(WaylandCapture {
            conn: connection(p)?,
            windows: false,
        }))
    },
};

struct ForeignToplevels(Arc<Connection>);

impl WindowList for ForeignToplevels {
    fn snapshot(&self) -> Vec<WindowInfo> {
        self.0.windows()
    }
    fn has_geometry(&self) -> bool {
        false
    }
    fn events(&self) -> async_channel::Receiver<WindowEvent> {
        self.0.window_events()
    }
}

pub const FOREIGN_TOPLEVEL_NO_GEOMETRY: Provider<dyn WindowList> = Provider {
    name: "foreign-toplevel",
    probe: |p| {
        (p.globals.ext_foreign_toplevel_list.is_some() || p.globals.wlr_foreign_toplevel.is_some())
            .then_some(1)
    },
    build: |p| Ok(Box::new(ForeignToplevels(connection(p)?))),
};

struct DataControl(Arc<Connection>);

impl Clipboard for DataControl {
    fn set(&self, items: Vec<ClipItem>) -> Result<()> {
        Ok(self.0.set_clipboard(&items)?)
    }
    fn needs_focused_window(&self) -> bool {
        false
    }
}

pub const DATA_CONTROL: Provider<dyn Clipboard> = Provider {
    name: "data-control",
    probe: |p| {
        (p.globals.ext_data_control.is_some() || p.globals.wlr_data_control.is_some()).then_some(10)
    },
    build: |p| Ok(Box::new(DataControl(connection(p)?))),
};

struct VirtualPointer(Arc<Connection>);

impl VirtualInput for VirtualPointer {
    fn scroll(&self, at: Point, dy: f64) -> Result<()> {
        Ok(self.0.scroll(at, dy)?)
    }
}

pub const VIRTUAL_POINTER: Provider<dyn VirtualInput> = Provider {
    name: "virtual-pointer",
    probe: |p| p.globals.virtual_pointer.is_some().then_some(10),
    build: |p| Ok(Box::new(VirtualPointer(connection(p)?))),
};

// A session streams cursor events for every output while it lives, so one is open only while a
// caller holds it.
struct StreamCursor(Arc<Connection>);

impl CursorSource for StreamCursor {
    fn sample(&self) -> Option<(Point, Instant)> {
        None
    }

    fn open(&self) -> Option<Arc<dyn CursorSource>> {
        let session: CursorSession = self
            .0
            .cursor_session()
            .inspect_err(|e| tracing::warn!("cursor session: {e}"))
            .ok()?;
        Some(Arc::new(session))
    }
}

struct SeatKeymap(Arc<Connection>);

impl KeymapSource for SeatKeymap {
    fn keymap(&self) -> Option<KeymapText> {
        self.0.keymap()
    }
}

pub fn keymap(p: &Probe) -> Option<Arc<dyn KeymapSource>> {
    let conn = p.wayland.clone()?;
    Some(Arc::new(SeatKeymap(conn)))
}

struct LockNotify(Arc<Connection>);

impl ScreenLock for LockNotify {
    fn watch(&self) -> async_channel::Receiver<bool> {
        self.0.screen_lock().unwrap_or_else(|| {
            let (tx, rx) = async_channel::bounded(1);
            let _ = tx.try_send(true);
            rx
        })
    }
}

pub const LOCK_NOTIFY: Provider<dyn ScreenLock> = Provider {
    name: "lock-notify",
    probe: |p| p.globals.lock_notify.map(|_| 10),
    build: |p| Ok(Box::new(LockNotify(connection(p)?))),
};

pub const CURSOR_SESSION: Provider<dyn CursorSource> = Provider {
    name: "cursor-session",
    probe: |p| {
        (p.globals.ext_image_copy_capture.is_some()
            && p.globals.ext_output_capture_source.is_some())
        .then_some(5)
    },
    build: |p| Ok(Box::new(StreamCursor(connection(p)?))),
};
