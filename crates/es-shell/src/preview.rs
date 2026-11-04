use std::path::Path;
use std::sync::Arc;

use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    App, AppContext as _, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ObjectFit, ParentElement, Render, RenderImage, StatefulInteractiveElement,
    Styled, StyledImage, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div,
    img, px, size,
};
use gpui_component::Root;

use crate::strings::fl;

const SCREEN_SHARE: f32 = 0.8;
const MIN_SIZE: (f32, f32) = (360., 240.);
const FALLBACK_SCREEN: (f32, f32) = (1280., 800.);

/// A window that fits the image inside `SCREEN_SHARE` of the screen without enlarging it.
pub fn initial_size(image: (u32, u32), screen: (f32, f32)) -> (f32, f32) {
    let (w, h) = (image.0 as f32, image.1 as f32);
    let fit = (screen.0 * SCREEN_SHARE / w)
        .min(screen.1 * SCREEN_SHARE / h)
        .min(1.);
    ((w * fit).max(MIN_SIZE.0), (h * fit).max(MIN_SIZE.1))
}

/// The full-size look at one capture: no tools, closes on Space or Escape.
pub struct Preview {
    image: Arc<RenderImage>,
    pixels: (u32, u32),
    actual_size: bool,
    focus: FocusHandle,
}

impl Focusable for Preview {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Preview {
    pub fn new(
        image: Arc<RenderImage>,
        pixels: (u32, u32),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            image,
            pixels,
            actual_size: false,
            focus,
        }
    }

    pub fn actual_size(&self) -> bool {
        self.actual_size
    }

    pub fn toggle_actual_size(&mut self, cx: &mut Context<Self>) {
        self.actual_size = !self.actual_size;
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "space" | "escape" => window.remove_window(),
            "z" => self.toggle_actual_size(cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let body = if self.actual_size {
            let scale = window.scale_factor();
            let (w, h) = self.pixels;
            div()
                .id("preview-scroll")
                .size_full()
                .overflow_scroll()
                .child(
                    div()
                        .flex()
                        .min_w_full()
                        .min_h_full()
                        .items_center()
                        .justify_center()
                        .child(
                            img(self.image.clone())
                                .flex_none()
                                .w(px(w as f32 / scale))
                                .h(px(h as f32 / scale)),
                        ),
                )
        } else {
            div().id("preview-fit").size_full().child(
                img(self.image.clone())
                    .size_full()
                    .object_fit(ObjectFit::Contain),
            )
        };
        div()
            .id("preview")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_actual_size(cx)))
            .size_full()
            .bg(t.window)
            .text_color(t.text)
            .font_family(UI_FONT)
            .child(body)
    }
}

/// GPUI images are straight-alpha BGRA.
fn load(file: &Path) -> Option<(Arc<RenderImage>, (u32, u32))> {
    let mut rgba = es_store::decode::open_image(file).ok()?.into_rgba8();
    let pixels = rgba.dimensions();
    for px in rgba.pixels_mut() {
        px.0.swap(0, 2);
    }
    Some((
        Arc::new(RenderImage::new([image::Frame::new(rgba)])),
        pixels,
    ))
}

/// Opens a plain window on the capture; `None` when the file cannot be read as an image.
pub fn open(file: &Path, cx: &mut App) -> Option<gpui::Result<WindowHandle<Root>>> {
    let (image, pixels) = load(file)?;
    let screen = cx.primary_display().map_or(FALLBACK_SCREEN, |d| {
        let s = d.bounds().size;
        (f32::from(s.width), f32::from(s.height))
    });
    let (w, h) = initial_size(pixels, screen);
    let name = file
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(fl!("preview-window-title", name = name).into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(w), px(h)), cx)),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        app_id: Some(es_protocol::PREVIEW_APP_ID.into()),
        ..Default::default()
    };
    Some(cx.open_window(options, move |window, cx| {
        let view = cx.new(|cx| Preview::new(image, pixels, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    }))
}
