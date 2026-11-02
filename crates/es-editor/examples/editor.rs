use std::path::PathBuf;

use es_editor::{EditorSource, EditorWindow};
use gpui::{
    App, AppContext, Bounds, WindowBounds, WindowDecorations, WindowOptions, point, px, size,
};
use gpui_component::{Root, TitleBar};

fn main() {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: editor <image>");
        std::process::exit(2);
    };
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(move |cx: &mut App| {
            es_ui::components::gallery::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1080.), px(720.)),
                ))),
                window_min_size: Some(size(px(980.), px(640.))),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("es-editor-window".into()),
                ..TitleBar::window_options()
            };
            let opened = cx.open_window(options, |window, cx| {
                let editor = cx.new(|cx| EditorWindow::open(EditorSource::File(path), cx));
                cx.new(|cx| Root::new(editor, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("cannot open the editor window: {error}");
                cx.quit();
            }
        });
}
