use crate::Error;
use crate::worker::State;
use es_geom::{Point, Rect};
use es_model::platform::CursorSource;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;
use wayland_client::protocol::wl_pointer::{Axis, AxisSource, WlPointer};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::image_capture_source::v1::client::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_cursor_session_v1::{
    self, ExtImageCopyCaptureCursorSessionV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

const SUBPIXEL: f64 = 16.0;
const WHEEL_STEP: f64 = 15.0;

type Sample = Mutex<Option<(Point, Instant)>>;

#[derive(Clone)]
pub struct CursorSession(Arc<Sample>);

impl CursorSource for CursorSession {
    fn sample(&self) -> Option<(Point, Instant)> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

struct CursorTracker {
    shared: Weak<Sample>,
    pointer: WlPointer,
    sessions: Vec<(ExtImageCopyCaptureCursorSessionV1, ExtImageCaptureSourceV1)>,
}

pub(crate) struct Input {
    pub pointer_capability: bool,
    pub virtual_manager: Option<ZwlrVirtualPointerManagerV1>,
    virtual_pointer: Option<ZwlrVirtualPointerV1>,
    cursor: Option<CursorTracker>,
    // Wayland timestamps are 32-bit milliseconds from an arbitrary base and wrap by design.
    epoch: Instant,
}

impl Input {
    pub fn new() -> Self {
        Input {
            pointer_capability: false,
            virtual_manager: None,
            virtual_pointer: None,
            cursor: None,
            epoch: Instant::now(),
        }
    }
}

fn absolute(at: Point, outputs: &[Rect]) -> Option<[u32; 4]> {
    if !outputs.iter().any(|r| r.contains(at)) {
        return None;
    }
    let layout = outputs.iter().copied().reduce(|a, b| a.union(b))?;
    let fixed = |v: f64| (v * SUBPIXEL).round() as u32;
    Some([
        fixed(at.x - layout.x0),
        fixed(at.y - layout.y0),
        fixed(layout.width()),
        fixed(layout.height()),
    ])
}

fn wheel_steps(dy: f64) -> i32 {
    match (dy / WHEEL_STEP).round() as i32 {
        0 if dy != 0.0 => dy.signum() as i32,
        n => n,
    }
}

impl State {
    fn output_rects(&self) -> Vec<Rect> {
        self.outputs
            .iter()
            .filter_map(|o| o.announced.as_ref().map(|i| i.logical))
            .collect()
    }

    pub(crate) fn scroll(
        &mut self,
        at: Point,
        dy: f64,
        park: Option<Point>,
        qh: &QueueHandle<State>,
    ) -> Result<(), Error> {
        let outputs = self.output_rects();
        let [x, y, x_extent, y_extent] = absolute(at, &outputs).ok_or(Error::OffScreen)?;
        let parked = park.and_then(|p| absolute(p, &outputs));
        let input = &mut self.input;
        let manager = input
            .virtual_manager
            .as_ref()
            .ok_or(Error::Unsupported("zwlr_virtual_pointer_manager_v1"))?;
        let seat = self.seat.as_ref();
        let pointer = input
            .virtual_pointer
            .get_or_insert_with(|| manager.create_virtual_pointer(seat, qh, ()));
        let time = input.epoch.elapsed().as_millis() as u32;
        pointer.motion_absolute(time, x, y, x_extent, y_extent);
        pointer.frame();
        pointer.axis_source(AxisSource::Wheel);
        pointer.axis_discrete(time, Axis::VerticalScroll, dy, wheel_steps(dy));
        pointer.frame();
        if let Some([x, y, x_extent, y_extent]) = parked {
            pointer.motion_absolute(time, x, y, x_extent, y_extent);
            pointer.frame();
        }
        Ok(())
    }

    pub(crate) fn cursor_session(
        &mut self,
        qh: &QueueHandle<State>,
    ) -> Result<CursorSession, Error> {
        if let Some(shared) = self.input.cursor.as_ref().and_then(|c| c.shared.upgrade()) {
            return Ok(CursorSession(shared));
        }
        self.stop_cursor();
        let (Some(copy), Some(sources)) = (&self.ext_copy, &self.ext_output_source) else {
            return Err(Error::Unsupported("ext-image-copy-capture cursor sessions"));
        };
        let seat = self.seat.as_ref().ok_or(Error::Unsupported("wl_seat"))?;
        if !self.input.pointer_capability {
            return Err(Error::Unsupported("a pointer on the seat"));
        }
        let pointer = seat.get_pointer(qh, ());
        let sessions = self
            .outputs
            .iter()
            .filter(|o| o.announced.is_some())
            .map(|o| {
                let source = sources.create_source(&o.wl, qh, ());
                let session = copy.create_pointer_cursor_session(&source, &pointer, qh, o.global);
                (session, source)
            })
            .collect();
        let shared = Arc::new(Mutex::new(None));
        self.input.cursor = Some(CursorTracker {
            shared: Arc::downgrade(&shared),
            pointer,
            sessions,
        });
        Ok(CursorSession(shared))
    }

    fn stop_cursor(&mut self) {
        if let Some(c) = self.input.cursor.take() {
            for (session, source) in c.sessions {
                session.destroy();
                source.destroy();
            }
            if c.pointer.version() >= 3 {
                c.pointer.release();
            }
        }
    }

    fn cursor_moved(&mut self, output: u32, x: i32, y: i32) {
        let Some(shared) = self.input.cursor.as_ref().and_then(|c| c.shared.upgrade()) else {
            self.stop_cursor();
            return;
        };
        let Some(info) = self
            .outputs
            .iter()
            .find(|o| o.global == output)
            .and_then(|o| o.announced.as_ref())
        else {
            return;
        };
        let at = Point::new(
            info.logical.x0 + f64::from(x) / info.scale,
            info.logical.y0 + f64::from(y) / info.scale,
        );
        *shared.lock().unwrap_or_else(|e| e.into_inner()) = Some((at, Instant::now()));
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            state.input.pointer_capability = caps.contains(wl_seat::Capability::Pointer);
            let keyboard = caps.contains(wl_seat::Capability::Keyboard);
            state.keyboard.set_capability(keyboard, seat, qh);
        }
    }
}

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, u32> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureCursorSessionV1,
        event: ext_image_copy_capture_cursor_session_v1::Event,
        output: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_image_copy_capture_cursor_session_v1::Event::Position { x, y } = event {
            state.cursor_moved(*output, x, y);
        } else if state
            .input
            .cursor
            .as_ref()
            .is_some_and(|c| c.shared.strong_count() == 0)
        {
            state.stop_cursor();
        }
    }
}

crate::capture::no_events!(WlPointer, ZwlrVirtualPointerManagerV1, ZwlrVirtualPointerV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_motion_is_relative_to_the_whole_layout() {
        let outputs = [
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            Rect::new(1280.0, -200.0, 3200.0, 880.0),
        ];
        let [x, y, xe, ye] = absolute(Point::new(1300.5, 10.0), &outputs).expect("on screen");
        assert_eq!((xe, ye), (3200 * 16, 1080 * 16));
        assert!((f64::from(x) / f64::from(xe) * 3200.0 - 1300.5).abs() < 0.1);
        assert!((f64::from(y) / f64::from(ye) * 1080.0 - 210.0).abs() < 0.1);
    }

    #[test]
    fn a_point_in_a_gap_between_outputs_is_refused() {
        let outputs = [
            Rect::new(0.0, 0.0, 1280.0, 720.0),
            Rect::new(1280.0, -200.0, 3200.0, 880.0),
        ];
        assert_eq!(absolute(Point::new(10.0, -100.0), &outputs), None);
        assert_eq!(absolute(Point::new(10.0, 10.0), &[]), None);
    }

    #[test]
    fn every_nonzero_scroll_moves_at_least_one_wheel_step() {
        assert_eq!(wheel_steps(-150.0), -10);
        assert_eq!(wheel_steps(3.0), 1);
        assert_eq!(wheel_steps(-3.0), -1);
        assert_eq!(wheel_steps(0.0), 0);
    }
}
