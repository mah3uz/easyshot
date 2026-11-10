use super::geometry;
use super::scroll::{self, Command, Outcome, Params, Source, Update};
use super::scroll_hud::{
    HUD, HudEvent, PREVIEW_WIDTH, ScrollHud, ScrollPreview, Strip, hud_origin, hud_park, outline,
    preview_column,
};
use es_app::capture_coordinator::settle;
use es_app::errors::{Error, Result};
use es_app::platform_ui::{
    LayerShellSurfaces, LayerSurfaceHandle, Selection, SurfaceHandle as _, SurfaceRole,
};
use es_backend::{Backend, VirtualInput};
use es_geom::{Point, Rect, Size};
use es_model::platform::{CaptureOpts, Frame, OutputId, Transform, WindowId};
use gpui::{App, AppContext as _, AsyncApp, Entity, RenderImage};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

// The axis value of one wheel click on Wayland; how far a click moves the page is up to the app.
const WHEEL_CLICK: f64 = 15.0;

/// The running capture's command line, so the shortcut pressed again can stop it.
pub type Control = Rc<RefCell<Option<mpsc::Sender<Command>>>>;

struct Live {
    backend: Arc<Backend>,
    output: OutputId,
    /// Relative to the output.
    local: Rect,
    area: Rect,
    /// Where the pointer rests after each wheel step, off the captured area.
    park: Point,
    window: Option<WindowId>,
    /// False once the compositor has refused the window's own buffer.
    own_buffer: bool,
}

fn scroll_frame(frame: &Frame) -> Option<es_scroll::Frame> {
    let (w, h) = frame.size_px;
    es_scroll::Frame::new(w as usize, h as usize, frame.bgra.to_vec())
}

fn grab_area(backend: &Backend, output: &OutputId, local: Rect) -> Option<es_scroll::Frame> {
    let frames = backend
        .capture
        .capture_outputs(std::slice::from_ref(output), CaptureOpts { cursor: false })
        .inspect_err(|error| tracing::warn!(%error, "scroll capture frame"))
        .ok()?;
    scroll_frame(&geometry::crop(frames.first()?, local)?)
}

/// The window's own pixels, so windows on top of it stay out; its on-screen rectangle once they are refused.
fn grab_window<E: std::fmt::Display>(
    own_buffer: &mut bool,
    own: std::result::Result<Frame, E>,
    rectangle: impl FnOnce() -> Option<es_scroll::Frame>,
) -> Option<es_scroll::Frame> {
    match own {
        Ok(frame) => scroll_frame(&frame),
        Err(error) => {
            tracing::info!(%error, "scroll capture reads the window's rectangle");
            *own_buffer = false;

            rectangle()
        }
    }
}

/// Parks the pointer afterwards: this compositor may draw the cursor into the frame, and hover effects freeze into it.
fn wheel(input: &dyn VirtualInput, area: Rect, park: Point, clicks: i32) -> bool {
    let dy = f64::from(clicks) * WHEEL_CLICK;
    input.scroll(area.center(), dy, Some(park)).is_ok()
}

impl Source for Live {
    fn grab(&mut self) -> Option<es_scroll::Frame> {
        let Some(id) = self.window.as_ref().filter(|_| self.own_buffer) else {
            return grab_area(&self.backend, &self.output, self.local);
        };
        let own = self
            .backend
            .capture
            .capture_window(id, CaptureOpts { cursor: false });
        let (backend, output, local) = (&self.backend, &self.output, self.local);
        grab_window(&mut self.own_buffer, own, || {
            grab_area(backend, output, local)
        })
    }

    fn scroll(&mut self, clicks: i32) -> bool {
        self.backend
            .virtual_input
            .as_deref()
            .is_some_and(|input| wheel(input, self.area, self.park, clicks))
    }

    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }

    fn now(&self) -> Instant {
        Instant::now()
    }
}

enum Message {
    Update(Update),
    Done(Outcome),
}

/// Closes the session's surfaces and forgets its command line however it ends.
struct Shown {
    handles: Vec<LayerSurfaceHandle>,
    control: Control,
    cx: AsyncApp,
}

impl Drop for Shown {
    fn drop(&mut self) {
        self.control.borrow_mut().take();
        let handles = std::mem::take(&mut self.handles);
        self.cx.update(|cx| {
            for handle in &handles {
                handle.close(cx);
            }
        });
    }
}

fn place(
    surfaces: &LayerShellSurfaces,
    output: &OutputId,
    rect: Rect,
    cx: &mut App,
) -> Result<LayerSurfaceHandle> {
    let handle = surfaces.open_surface(
        SurfaceRole::Scroll,
        output,
        Size::new(rect.width(), rect.height()),
        cx,
    )?;
    handle.set_position(Point::new(rect.x0, rect.y0), cx);
    Ok(handle)
}

fn render_image(frame: es_scroll::Frame) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(frame.width as u32, frame.height as u32, frame.data)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

pub(crate) async fn capture(
    surfaces: Rc<LayerShellSurfaces>,
    backend: Arc<Backend>,
    control: Control,
    output: OutputId,
    local: Rect,
    window: Option<WindowId>,
    cx: &mut AsyncApp,
) -> Result<Selection> {
    settle(cx).await;
    let info = surfaces.output_info(&output)?;
    let (visible, scale) = (info.logical, info.scale);
    let area = local + Point::new(visible.x0, visible.y0).to_vec2();
    let auto_available = backend.virtual_input.is_some();
    let column = preview_column(area, visible);
    let (tx, rx) = mpsc::channel();
    *control.borrow_mut() = Some(tx.clone());
    let mut shown = Shown {
        handles: Vec::new(),
        control: control.clone(),
        cx: cx.clone(),
    };
    let preview_px = column.map_or((1, 1), |c| {
        (
            (PREVIEW_WIDTH * scale) as usize,
            (c.height() * scale) as usize,
        )
    });
    let params = Params {
        auto_available,
        preview: preview_px,
        max_height: es_scroll::MAX_HEIGHT,
    };
    let mut source = Live {
        backend,
        output: output.clone(),
        local,
        area,
        park: hud_park(hud_origin(area, visible), auto_available),
        window,
        own_buffer: true,
    };
    let (out, messages) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("scroll-capture".into())
        .spawn(move || {
            let mut emit = |u| {
                let _ = out.send_blocking(Message::Update(u));
            };
            let outcome = scroll::run(&mut source, &rx, params, &mut emit);
            let _ = out.send_blocking(Message::Done(outcome));
        })
        .map_err(|e| Error::Internal(format!("scroll capture thread: {e}")))?;
    let (hud, preview) = cx.update(|cx| -> Result<_> {
        for strip in outline(area, visible) {
            let handle = place(&surfaces, &output, strip, cx)?;
            handle.set_content(cx.new(|_| Strip).into(), cx);
            shown.handles.push(handle);
        }
        let preview = match column {
            Some(rect) => {
                let handle = place(&surfaces, &output, rect, cx)?;
                let view = cx.new(|_| ScrollPreview::new());
                handle.set_content(view.clone().into(), cx);
                shown.handles.push(handle);
                Some(view)
            }
            None => None,
        };
        let origin = hud_origin(area, visible);
        let rect = Rect::new(origin.x, origin.y, origin.x + HUD.0, origin.y + HUD.1);
        let handle = place(&surfaces, &output, rect, cx)?;
        handle.set_input_region(&[Rect::new(0.0, 0.0, HUD.0, HUD.1)], cx);
        let hud = cx.new(|_| ScrollHud::new(auto_available));
        handle.set_content(hud.clone().into(), cx);
        shown.handles.push(handle);
        Ok((hud, preview))
    })?;
    let _events = cx.update(|cx| {
        cx.subscribe(&hud, move |_, event: &HudEvent, _| {
            let _ = tx.send(match *event {
                HudEvent::Cancel => Command::Cancel,
                HudEvent::Stop => Command::Stop,
                HudEvent::Auto(on) => Command::Auto(on),
            });
        })
    });
    let width = area.width().round() as u32;
    let outcome = loop {
        match messages.recv().await.map_err(|_| Error::Cancelled)? {
            Message::Done(outcome) => break outcome,
            Message::Update(update) => {
                cx.update(|cx| apply(update, &hud, preview.as_ref(), width, scale, cx))
            }
        }
    };
    drop(shown);
    match outcome {
        Outcome::Done(f) => {
            let logical = Rect::new(
                area.x0,
                area.y0,
                area.x0 + f.width as f64 / scale,
                area.y0 + f.height as f64 / scale,
            );
            Ok(Selection::Region {
                output: output.clone(),
                logical,
                frame: Arc::new(Frame {
                    output: Some(output),
                    size_px: (f.width as u32, f.height as u32),
                    logical,
                    scale,
                    bgra: Arc::from(f.data),
                    transform: Transform::Normal,
                    captured_at: Instant::now(),
                }),
            })
        }
        Outcome::Cancelled => Ok(Selection::Cancelled),
        Outcome::Failed => Err(Error::Protocol("the scroll capture got no frames".into())),
    }
}

fn apply(
    update: Update,
    hud: &Entity<ScrollHud>,
    preview: Option<&Entity<ScrollPreview>>,
    width: u32,
    scale: f64,
    cx: &mut App,
) {
    match update {
        Update::Height(h) => hud.update(cx, |hud, cx| {
            hud.set_size(width, (h as f64 / scale).round() as u32, cx)
        }),
        Update::Auto(on) => hud.update(cx, |hud, cx| hud.set_auto(on, cx)),
        Update::Preview(frame) => {
            let (w, h) = (frame.width as f64 / scale, frame.height as f64 / scale);
            if let (Some(view), Some(image)) = (preview, render_image(frame)) {
                view.update(cx, |p, cx| p.set_image(image, w, h, cx));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_frame() -> Frame {
        Frame {
            output: None,
            size_px: (2, 1),
            logical: Rect::new(0.0, 0.0, 2.0, 1.0),
            scale: 1.0,
            bgra: Arc::from(vec![1u8; 8]),
            transform: Transform::Normal,
            captured_at: Instant::now(),
        }
    }

    fn rectangle_frame() -> Option<es_scroll::Frame> {
        es_scroll::Frame::new(1, 1, vec![9; 4])
    }

    use std::sync::Mutex;

    struct Record(Mutex<Vec<(Point, f64, Option<Point>)>>);

    impl VirtualInput for Record {
        fn scroll(&self, at: Point, dy: f64, park: Option<Point>) -> es_backend::Result<()> {
            self.0.lock().expect("lock").push((at, dy, park));
            Ok(())
        }
    }

    #[test]
    fn auto_scroll_leaves_the_pointer_on_the_bar_outside_the_area() {
        let visible = Rect::new(0.0, 0.0, 1280.0, 720.0);
        let area = Rect::new(400.0, 100.0, 800.0, 500.0);
        for auto in [true, false] {
            let park = hud_park(hud_origin(area, visible), auto);
            let input = Record(Mutex::default());
            assert!(wheel(&input, area, park, 2));
            let sent = input.0.lock().expect("lock").clone();
            assert_eq!(sent, vec![(area.center(), 30.0, Some(park))]);
            assert!(!area.contains(park), "the pointer would sit in the picture");
        }
    }

    #[test]
    fn a_window_that_gives_its_own_pixels_is_read_from_them_not_from_the_screen() {
        let mut own_buffer = true;
        let got = grab_window(&mut own_buffer, Ok::<_, String>(window_frame()), || {
            panic!("the screen is not read")
        });
        assert_eq!(got.map(|f| (f.width, f.height)), Some((2, 1)));
        assert!(own_buffer);
    }

    #[test]
    fn a_refused_window_buffer_falls_back_to_the_rectangle_and_stays_there() {
        let mut own_buffer = true;
        let got = grab_window(&mut own_buffer, Err("no buffer"), rectangle_frame);
        assert_eq!(got.map(|f| (f.width, f.height)), Some((1, 1)));
        assert!(!own_buffer);
    }
}
