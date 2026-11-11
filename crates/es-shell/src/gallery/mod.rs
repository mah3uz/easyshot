mod cells;
pub mod files;
mod menu;
pub mod model;
mod render;
pub mod shared;
mod size;
pub mod thumbs;
mod view;

pub use files::{DesktopFiles, FileResult, GalleryFiles};
pub use model::{Category, Item, MediaKind, Section, SortOrder, Step, ViewMode};
pub use shared::ShareAction;
pub use view::{Gallery, GalleryEvent};

use crate::strings::fl;
use es_app::history::History;
use es_app::settings_model::SettingsModel;
use es_store::Paths;
use gpui::{
    App, AppContext, Bounds, Entity, Focusable, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions, size,
};
use gpui_component::Root;
use std::sync::Arc;

pub const WINDOW_SIZE: (f32, f32) = (1080., 740.);
pub const WINDOW_MIN_SIZE: (f32, f32) = (780., 560.);

pub fn root(
    history: Entity<History>,
    settings: Entity<SettingsModel>,
    paths: Paths,
    files: Arc<dyn GalleryFiles>,
    window: &mut gpui::Window,
    cx: &mut App,
) -> (Entity<Root>, Entity<Gallery>) {
    let gallery = cx.new(|cx| Gallery::new(history, settings, paths, files, window, cx));
    gallery.read(cx).focus_handle(cx).focus(window, cx);
    let root = cx.new(|cx| Root::new(gallery.clone(), window, cx));
    (root, gallery)
}

pub fn open(
    history: Entity<History>,
    settings: Entity<SettingsModel>,
    paths: Paths,
    files: Arc<dyn GalleryFiles>,
    on_event: impl Fn(&GalleryEvent, &mut App) + 'static,
    cx: &mut App,
) -> gpui::Result<WindowHandle<Root>> {
    let (w, h) = WINDOW_SIZE;
    let (min_w, min_h) = WINDOW_MIN_SIZE;
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(fl!("gallery-window-title").into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(gpui::px(w), gpui::px(h)),
            cx,
        ))),
        window_min_size: Some(size(gpui::px(min_w), gpui::px(min_h))),
        app_id: Some(es_protocol::GALLERY_APP_ID.into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        let (root, gallery) = root(history, settings, paths, files, window, cx);
        cx.subscribe(&gallery, move |_, event, cx| on_event(event, cx))
            .detach();
        root
    })
}
