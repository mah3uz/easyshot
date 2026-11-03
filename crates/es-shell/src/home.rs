use crate::gallery::{Gallery, GalleryFiles};
use crate::settings::{Page, SettingsDeps, SettingsWindow};
use crate::strings::fl;
use es_app::history::History;
use es_store::Paths;
use es_ui::{
    a11y::{Accessible, Role},
    components::{Segment, Segmented},
    theme::{UI_FONT, tokens},
};
use gpui::{
    App, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div, px,
    size,
};
use gpui_component::Root;
use std::sync::Arc;

pub const WINDOW_SIZE: (f32, f32) = (1080., 740.);
pub const MIN_SIZE: (f32, f32) = (780., 664.);
pub const TAB_BAR_HEIGHT: f32 = 44.;
const SWITCHER_WIDTH: f32 = 220.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Gallery,
    Settings,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Gallery, Tab::Settings];

    pub fn title(self) -> String {
        match self {
            Tab::Gallery => fl!("home-window-gallery"),
            Tab::Settings => fl!("home-window-settings"),
        }
    }
}

pub struct HomeDeps {
    pub history: Entity<History>,
    pub paths: Paths,
    pub files: Arc<dyn GalleryFiles>,
    pub settings: SettingsDeps,
}

pub struct HomeWindow {
    tab: Tab,
    gallery: Entity<Gallery>,
    settings: Entity<SettingsWindow>,
    focus_gallery: bool,
}

impl HomeWindow {
    pub fn new(deps: HomeDeps, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let gallery = cx.new(|cx| Gallery::new(deps.history, deps.paths, deps.files, window, cx));
        let settings = cx.new(|cx| SettingsWindow::new(deps.settings, window, cx));
        Self {
            tab: Tab::Gallery,
            gallery,
            settings,
            focus_gallery: true,
        }
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn gallery(&self) -> &Entity<Gallery> {
        &self.gallery
    }

    pub fn settings(&self) -> &Entity<SettingsWindow> {
        &self.settings
    }

    pub fn show(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.focus_gallery |= tab == Tab::Gallery && self.tab != tab;
        self.tab = tab;
        cx.notify();
    }

    /// `None` keeps whichever page Settings last showed.
    pub fn show_settings(&mut self, page: Option<Page>, cx: &mut Context<Self>) {
        if let Some(page) = page {
            self.settings.update(cx, |s, cx| s.show(page, cx));
        }
        self.show(Tab::Settings, cx);
    }

    fn tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx);
        let view = cx.entity().downgrade();
        let switcher = Segmented::new(
            "home-tabs",
            fl!("home-window-tabs"),
            Tab::ALL.map(|tab| Segment::text(tab.title())),
        )
        .selected(Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0))
        .height(px(28.))
        .on_select(move |index, _, cx| {
            let tab = Tab::ALL.get(index).copied().unwrap_or(Tab::Gallery);
            let _ = view.update(cx, |this, cx| this.show(tab, cx));
        });
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(TAB_BAR_HEIGHT))
            .bg(t.inspector)
            .border_b_1()
            .border_color(t.border)
            .child(div().w(px(SWITCHER_WIDTH)).child(switcher))
    }
}

impl Render for HomeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_gallery) {
            self.gallery.read(cx).focus_handle(cx).focus(window, cx);
        }
        let t = tokens(cx);
        let content = match self.tab {
            Tab::Gallery => self.gallery.clone().into_any_element(),
            Tab::Settings => self.settings.clone().into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(UI_FONT)
            .bg(t.window)
            .text_color(t.text)
            .child(self.tab_bar(cx))
            .child(
                div()
                    .id("home-content")
                    .accessible(Role::TabPanel, self.tab.title())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(div().absolute().inset_0().child(content)),
            )
    }
}

pub fn root(
    deps: HomeDeps,
    tab: Tab,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<Root>, Entity<HomeWindow>) {
    let home = cx.new(|cx| {
        let mut home = HomeWindow::new(deps, window, cx);
        home.show(tab, cx);
        home
    });
    let root = cx.new(|cx| Root::new(home.clone(), window, cx));
    (root, home)
}

/// Uses the app's own id, so the window matches the desktop entry that launches it.
pub fn open(deps: HomeDeps, cx: &mut App) -> gpui::Result<WindowHandle<Root>> {
    let (w, h) = WINDOW_SIZE;
    let (min_w, min_h) = MIN_SIZE;
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(fl!("home-window-title").into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(w), px(h)), cx)),
        window_min_size: Some(size(px(min_w), px(min_h))),
        app_id: Some(es_protocol::APP_ID.into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| root(deps, Tab::Gallery, window, cx).0)
}
