#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "../tests/it/canvas_support/mod.rs"]
mod canvas_support;

use canvas_support::{TEXT, every_kind, opened};
use es_editor::{EditorCanvas, EditorSession};
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::GradientPreset;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, point, px, size};

fn main() {
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(|cx: &mut App| {
            es_ui::init(cx);
            let look = BackgroundSettings {
                style: GradientPreset::by_id("soft-lilac")
                    .map(|g| BackgroundStyle::Gradient(g.style()))
                    .unwrap_or_default(),
                ..BackgroundSettings::default()
            };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1100.), px(700.)),
                ))),
                app_id: Some("es-editor-canvas".into()),
                ..WindowOptions::default()
            };
            let opened = cx.open_window(options, |window, cx| {
                let session = EditorSession::new(
                    opened(1200, 640, every_kind(true, true), look),
                    TEXT.clone(),
                    cx,
                );
                let canvas = cx.new(|cx| EditorCanvas::new(&session, cx));
                let focus = canvas.read(cx).focus_handle().clone();
                window.focus(&focus, cx);
                canvas
            });
            if let Err(error) = opened {
                eprintln!("cannot open the canvas window: {error}");
                cx.quit();
            }
        });
}
