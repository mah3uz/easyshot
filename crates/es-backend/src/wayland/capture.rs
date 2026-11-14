use crate::error::Result;
use crate::traits::{Capture, WindowList};
use es_geom::{Rect, Size};
use es_model::platform::{
    CaptureOpts, Frame, OutputEvent, OutputId, OutputInfo, Transform, WindowId,
};
use es_wayland::Connection;
use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
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

/// A window shot has the size a crop of the screen would give: wlroots renders a window capture at
/// scale 1, which on a scaled output holds fewer pixels, so it is scaled up.
pub fn fit_window_frame(
    mut frame: Frame,
    logical: Option<Rect>,
    size: Option<Size>,
    output_scale: Option<f64>,
) -> Frame {
    let Some(size) = logical
        .map(|r| r.size())
        .or(size)
        .filter(|s| s.width > 0.0 && s.height > 0.0)
    else {
        return frame;
    };
    if let Some(up) = output_scale.and_then(|s| scaled_up(&frame, size * s)) {
        frame = up;
    }
    frame.scale = f64::from(frame.size_px.0) / size.width;
    frame.logical = logical.unwrap_or_else(|| Rect::from_origin_size(frame.logical.origin(), size));
    frame
}

fn scaled_up(frame: &Frame, want: Size) -> Option<Frame> {
    let want = want.round();
    if want.width <= f64::from(frame.size_px.0) + 1.0 || want.width > f64::from(u16::MAX) {
        return None;
    }
    let (w, h) = (want.width as u32, want.height as u32);
    let (sw, sh) = frame.size_px;
    let src = ImageRef::new(sw, sh, &frame.bgra, PixelType::U8x4)
        .inspect_err(|e| tracing::warn!("window frame of {sw}x{sh}: {e}"))
        .ok()?;
    let mut dst = Image::new(w, h, PixelType::U8x4);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::CatmullRom));
    Resizer::new()
        .resize(&src, &mut dst, &options)
        .inspect_err(|e| tracing::warn!("window frame scaled to {w}x{h}: {e}"))
        .ok()?;
    Some(Frame {
        size_px: (w, h),
        bgra: dst.into_vec().into(),
        ..frame.clone()
    })
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
        let Some(info) = self.windows.snapshot().into_iter().find(|i| i.id == *w) else {
            return Ok(frame);
        };
        let size = info
            .logical
            .is_none()
            .then(|| self.windows.size(w))
            .flatten();
        let scale = info.output.and_then(|id| {
            let outputs = self.inner.outputs();
            outputs.iter().find(|o| o.id == id).map(|o| o.scale)
        });
        Ok(fit_window_frame(frame, info.logical, size, scale))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::time::Instant;

    fn frame(w: u32, h: u32, bgra: [u8; 4]) -> Frame {
        Frame {
            output: None,
            size_px: (w, h),
            logical: Rect::new(0.0, 0.0, f64::from(w), f64::from(h)),
            scale: 1.0,
            bgra: bgra.repeat((w * h) as usize).into(),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        }
    }

    // A window shot is as large as the screen shows it, the same as a crop of that window.
    #[test]
    fn a_window_captured_at_scale_one_is_scaled_up_to_its_output_scale() {
        let shown = Rect::new(100.0, 50.0, 500.0, 350.0);
        let fitted = fit_window_frame(
            frame(400, 300, [1, 2, 3, 255]),
            Some(shown),
            None,
            Some(1.5),
        );
        assert_eq!(fitted.size_px, (600, 450));
        assert_eq!(fitted.bgra.len(), 600 * 450 * 4);
        assert!(
            fitted.bgra.chunks(4).all(|p| p == [1, 2, 3, 255]),
            "the window's pixels"
        );
        assert_eq!(fitted.scale, 1.5);
        assert_eq!(fitted.logical, shown);
    }

    #[test]
    fn a_window_captured_at_the_output_scale_keeps_its_pixels() {
        let shown = Rect::new(0.0, 0.0, 400.0, 300.0);
        let captured = frame(600, 450, [9, 9, 9, 255]);
        let fitted = fit_window_frame(captured.clone(), Some(shown), None, Some(1.5));
        assert_eq!(fitted.size_px, (600, 450));
        assert!(Arc::ptr_eq(&fitted.bgra, &captured.bgra), "not resampled");
        assert_eq!(fitted.scale, 1.5);
    }

    // A window no screen shows has no rectangle; its size still says how many pixels a crop has.
    #[test]
    fn a_hidden_window_is_scaled_by_its_size_and_its_output_scale() {
        let size = Some(Size::new(400.0, 300.0));
        let fitted = fit_window_frame(frame(400, 300, [0; 4]), None, size, Some(2.0));
        assert_eq!(fitted.size_px, (800, 600));
        assert_eq!(fitted.scale, 2.0);
        assert_eq!(fitted.logical.size(), Size::new(400.0, 300.0));

        let unknown = fit_window_frame(frame(400, 300, [0; 4]), None, None, Some(2.0));
        assert_eq!((unknown.size_px, unknown.scale), ((400, 300), 1.0));
    }
}
