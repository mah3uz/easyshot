use crate::gallery::{Gallery, GalleryFiles};
use crate::settings::{Page, SettingsDeps, SettingsWindow};
use crate::strings::fl;
use es_app::history::History;
use es_store::Paths;
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton, Segment, Segmented},
    icons::IconName,
    motion::{Clocked, STEP_CHANGE, progress},
    theme::{UI_FONT, tokens},
};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, ExternalPaths, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Task, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions, div, px, size,
};
use gpui_component::Root;
use std::path::PathBuf;
use std::rc::Rc;
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

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn title(self) -> String {
        match self {
            Tab::Gallery => fl!("home-window-gallery"),
            Tab::Settings => fl!("home-window-settings"),
        }
    }
}

/// Asks for one image or video file; `None` when the user cancels.
pub type OpenPicker = Rc<dyn Fn(&mut App) -> Task<Option<PathBuf>>>;

pub enum HomeEvent {
    Open(PathBuf),
}

pub struct HomeDeps {
    pub history: Entity<History>,
    pub paths: Paths,
    pub files: Arc<dyn GalleryFiles>,
    pub settings: SettingsDeps,
    pub open_picker: Option<OpenPicker>,
}

pub struct HomeWindow {
    tab: Tab,
    tab_motion: Option<(Clocked, Tab)>,
    gallery: Entity<Gallery>,
    settings: Entity<SettingsWindow>,
    focus_gallery: bool,
    open_picker: Option<OpenPicker>,
}

impl EventEmitter<HomeEvent> for HomeWindow {}

impl HomeWindow {
    pub fn new(deps: HomeDeps, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let gallery = cx.new(|cx| {
            Gallery::new(
                deps.history,
                deps.settings.settings.clone(),
                deps.paths,
                deps.files,
                window,
                cx,
            )
        });
        let settings = cx.new(|cx| SettingsWindow::new(deps.settings, window, cx));
        Self {
            tab: Tab::Gallery,
            tab_motion: None,
            gallery,
            settings,
            focus_gallery: true,
            open_picker: deps.open_picker,
        }
    }

    pub fn choose_file(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.open_picker.clone() else {
            return;
        };
        let picked = picker(cx);
        cx.spawn(async move |this, cx| {
            if let Some(file) = picked.await {
                let _ = this.update(cx, |_, cx| cx.emit(HomeEvent::Open(file)));
            }
        })
        .detach();
    }

    pub fn drop_files(&mut self, files: &ExternalPaths, cx: &mut Context<Self>) {
        if let Some(file) = files.paths().first() {
            cx.emit(HomeEvent::Open(file.clone()));
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
        if tab != self.tab {
            self.tab_motion = Clocked::start(STEP_CHANGE, cx).map(|c| (c, self.tab));
        }
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
        .selected(self.tab.index())
        .glide()
        .height(px(28.))
        .on_select({
            let view = view.clone();
            move |index, _, cx| {
                let tab = Tab::ALL.get(index).copied().unwrap_or(Tab::Gallery);
                let _ = view.update(cx, |this, cx| this.show(tab, cx));
            }
        });
        let open = self.open_picker.is_some().then(|| {
            div().absolute().right(px(12.)).child(
                EditorButton::new("home-open")
                    .icon(IconName::FolderOpen)
                    .label(fl!("home-window-open"))
                    .tooltip(fl!("home-window-open-tip"))
                    .kind(ButtonKind::Bordered)
                    .on_click({
                        move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| this.choose_file(cx));
                        }
                    }),
            )
        });
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(TAB_BAR_HEIGHT))
            .bg(t.inspector)
            .border_b_1()
            .border_color(t.border)
            .child(div().w(px(SWITCHER_WIDTH)).child(switcher))
            .children(open)
    }
}

impl Render for HomeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.focus_gallery) {
            self.gallery.read(cx).focus_handle(cx).focus(window, cx);
        }
        let t = tokens(cx);
        let content = |tab: Tab| match tab {
            Tab::Gallery => self.gallery.clone().into_any_element(),
            Tab::Settings => self.settings.clone().into_any_element(),
        };
        let (arrived, from) = match self.tab_motion {
            Some((clock, from)) => (progress(Some(clock), window, cx), from),
            None => (1.0, self.tab),
        };
        let toward = if self.tab.index() < from.index() {
            -1.0
        } else {
            1.0
        };
        let width = window.viewport_size().width * toward;
        let leaving = (arrived < 1.0 && from != self.tab).then(|| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .w_full()
                .left(-width * arrived)
                .opacity(1.0 - arrived)
                .child(content(from))
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .font_family(UI_FONT)
            .bg(t.window)
            .text_color(t.text)
            .on_drop(cx.listener(|this, files: &ExternalPaths, _, cx| this.drop_files(files, cx)))
            .child(self.tab_bar(cx))
            .child(
                div()
                    .id("home-content")
                    .accessible(Role::TabPanel, self.tab.title())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .children(leaving)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .w_full()
                            .left(width * (1.0 - arrived))
                            .opacity(arrived)
                            .child(content(self.tab)),
                    ),
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
