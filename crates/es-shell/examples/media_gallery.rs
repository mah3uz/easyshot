use es_app::history::History;
use es_desktop::dbus::Endpoint;
use es_shell::gallery::{self, DesktopFiles, GalleryEvent};
use es_store::Paths;
use gpui::AppContext;
use std::sync::Arc;

fn main() {
    gpui_platform::application()
        .with_assets(es_ui::assets::Assets)
        .run(|cx| {
            es_ui::init(cx);
            let paths = match Paths::from_xdg() {
                Ok(paths) => paths,
                Err(error) => return eprintln!("no data directories: {error}"),
            };
            let history = match History::open(&paths, 0) {
                Ok(history) => cx.new(|_| history),
                Err(error) => return eprintln!("cannot read the library: {error}"),
            };
            let files = Arc::new(DesktopFiles::new(Endpoint::session()));
            let on_event = |event: &GalleryEvent, _: &mut gpui::App| eprintln!("{event:?}");
            if let Err(error) = gallery::open(history, paths, files, on_event, cx) {
                eprintln!("cannot open the gallery: {error}");
            }
            cx.activate(true);
        });
}
