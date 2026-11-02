use crate::error::Result;
use crate::traits::{Capture, WindowList};
use es_model::platform::{
    CaptureOpts, Frame, OutputEvent, OutputId, OutputInfo, Transform, WindowId, WindowInfo,
};
use es_wayland::Connection;
use std::sync::Arc;

pub(super) struct WaylandCapture {
    pub conn: Arc<Connection>,
    pub windows: bool,
}

impl Capture for WaylandCapture {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.conn.outputs()
    }

    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        self.conn.output_events()
    }

    fn capture_outputs(&self, ids: &[OutputId], o: CaptureOpts) -> Result<Vec<Frame>> {
        let frames = self.conn.capture_outputs(ids, o)?;
        Ok(frames.into_iter().map(normalise_frame).collect())
    }

    fn capture_window(&self, w: &WindowId, o: CaptureOpts) -> Result<Frame> {
        if !self.windows {
            return Err(crate::Error::Unsupported);
        }
        Ok(normalise_frame(self.conn.capture_toplevel(&w.0, o)?))
    }
}

fn swaps_axes(t: Transform) -> bool {
    matches!(
        t,
        Transform::R90 | Transform::R270 | Transform::F90 | Transform::F270
    )
}

/// `transform` is what the compositor applied to upright content: a mirror, then a counter-clockwise turn.
pub fn normalise_frame(mut frame: Frame) -> Frame {
    let t = frame.transform;
    if t == Transform::Normal {
        return frame;
    }
    let (bw, bh) = (frame.size_px.0 as usize, frame.size_px.1 as usize);
    if frame.bgra.len() != bw * bh * 4 {
        tracing::warn!(
            "frame of {}x{} has {} bytes; left unrotated",
            bw,
            bh,
            frame.bgra.len()
        );
        return frame;
    }
    let (w, h) = if swaps_axes(t) { (bh, bw) } else { (bw, bh) };
    let source = |x: usize, y: usize| -> (usize, usize) {
        match t {
            Transform::Normal => (x, y),
            Transform::R90 => (y, w - 1 - x),
            Transform::R180 => (w - 1 - x, h - 1 - y),
            Transform::R270 => (h - 1 - y, x),
            Transform::Flipped => (w - 1 - x, y),
            Transform::F90 => (y, x),
            Transform::F180 => (x, h - 1 - y),
            Transform::F270 => (h - 1 - y, w - 1 - x),
        }
    };
    let mut out = vec![0u8; frame.bgra.len()];
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = source(x, y);
            let from = (sy * bw + sx) * 4;
            let to = (y * w + x) * 4;
            out[to..to + 4].copy_from_slice(&frame.bgra[from..from + 4]);
        }
    }
    frame.bgra = out.into();
    frame.size_px = (w as u32, h as u32);
    frame.transform = Transform::Normal;
    frame
}

pub fn fit_window_frame(mut frame: Frame, window: &WindowInfo) -> Frame {
    if let Some(rect) = window.logical.filter(|r| r.width() > 0.0) {
        frame.scale = f64::from(frame.size_px.0) / rect.width();
        frame.logical = rect;
    }
    frame
}

pub struct WindowGeometry {
    pub inner: Box<dyn Capture>,
    pub windows: Arc<dyn WindowList>,
}

impl Capture for WindowGeometry {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.inner.outputs()
    }

    fn output_events(&self) -> async_channel::Receiver<OutputEvent> {
        self.inner.output_events()
    }

    fn capture_outputs(&self, ids: &[OutputId], o: CaptureOpts) -> Result<Vec<Frame>> {
        self.inner.capture_outputs(ids, o)
    }

    fn capture_window(&self, w: &WindowId, o: CaptureOpts) -> Result<Frame> {
        let frame = self.inner.capture_window(w, o)?;
        Ok(match self.windows.snapshot().iter().find(|i| i.id == *w) {
            Some(info) => fit_window_frame(frame, info),
            None => frame,
        })
    }
}
