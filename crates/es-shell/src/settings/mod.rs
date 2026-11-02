mod binds;
mod general;
mod home;
mod pages;
mod shortcuts;
mod widgets;

pub use binds::{BindStatus, compositor_trigger, from_backend, trigger_string};
pub use shortcuts::ShortcutRow;

use crate::strings::fl;
use es_app::history::History;
use es_app::settings_model::{SettingsEvent, SettingsModel};
use es_backend::{BindHelper, Caps};
use es_model::Config;
use es_protocol::Route;
use es_ui::{
    a11y::{Accessible, Role},
    components::{InspectorSlider, SliderEvent, ValueFormat},
    icons::IconName,
    theme::tokens,
};
use gpui::{
    App, AppContext, ClipboardItem, Context, Entity, EventEmitter, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowBounds, WindowHandle, WindowOptions, div, px, size,
};
use gpui_component::{
    Icon, Root, h_flex,
    input::{InputEvent, InputState},
    v_flex,
};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub const WINDOW_SIZE: (f32, f32) = (820., 660.);
pub const MIN_SIZE: (f32, f32) = (780., 620.);
pub const SIDEBAR_WIDTH: f32 = 210.;
const SIDEBAR_TOP: f32 = 44.;
const SIDEBAR_ROW: f32 = 28.;
const SIDEBAR_GROUP: f32 = 30.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Home,
    General,
    Capture,
    Overlay,
    Recording,
    Shortcuts,
    About,
}

impl Page {
    pub const ALL: [Page; 7] = [
        Page::Home,
        Page::General,
        Page::Capture,
        Page::Overlay,
        Page::Recording,
        Page::Shortcuts,
        Page::About,
    ];

    pub fn title(self) -> String {
        match self {
            Page::Home => fl!("settings-nav-home"),
            Page::General => fl!("settings-nav-general"),
            Page::Capture => fl!("settings-nav-capture"),
            Page::Overlay => fl!("settings-nav-overlay"),
            Page::Recording => fl!("settings-nav-recording"),
            Page::Shortcuts => fl!("settings-nav-shortcuts"),
            Page::About => fl!("settings-nav-about"),
        }
    }

    fn icon(self) -> IconName {
        match self {
            Page::Home => IconName::House,
            Page::General => IconName::Settings,
            Page::Capture => IconName::Camera,
            Page::Overlay => IconName::AppWindow,
            Page::Recording => IconName::Video,
            Page::Shortcuts => IconName::Keyboard,
            Page::About => IconName::Info,
        }
    }

    /// Where the page's row sits in the sidebar, for pointer tests and screenshots.
    pub fn sidebar_row_centre(self) -> (f32, f32) {
        let index = Page::ALL.iter().position(|p| *p == self).unwrap_or(0) as f32;
        let groups = match self {
            Page::Home => 0.,
            Page::About => 2.,
            _ => 1.,
        };
        (
            SIDEBAR_WIDTH / 2.,
            SIDEBAR_TOP + groups * SIDEBAR_GROUP + index * SIDEBAR_ROW + SIDEBAR_ROW / 2.,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsWindowEvent {
    Run(Route),
    ChooseSaveFolder,
    OpenRecent(PathBuf),
    ShowOnboarding,
    OpenUrl(&'static str),
}

pub trait ClipboardSink {
    fn copy(&self, text: String, cx: &mut App);
}

pub struct SystemClipboard;

impl ClipboardSink for SystemClipboard {
    fn copy(&self, text: String, cx: &mut App) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }
}

pub struct SettingsDeps {
    pub settings: Entity<SettingsModel>,
    pub history: Option<Entity<History>>,
    pub caps: Caps,
    pub binds: Option<Arc<dyn BindHelper>>,
    pub clipboard: Rc<dyn ClipboardSink>,
    /// Output names for "A specific screen".
    pub outputs: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    ShowBarAtLaunch,
    EditorsFullscreen,
    AutoSave,
    CopyAfterSave,
    PlaySound,
    CaptureOnRelease,
    Preselect,
    OpenEditor,
    KeepPreviews,
    AlwaysShowActions,
    SystemAudio,
    Keystrokes,
    RecordingOpenEditor,
    RecordingSave,
}

impl Toggle {
    pub fn get(self, c: &Config) -> bool {
        *Self::field(self, &mut c.clone())
    }

    fn field(self, c: &mut Config) -> &mut bool {
        match self {
            Toggle::ShowBarAtLaunch => &mut c.general.show_capture_bar_at_launch,
            Toggle::EditorsFullscreen => &mut c.general.editor_opens_fullscreen,
            Toggle::AutoSave => &mut c.general.auto_save,
            Toggle::CopyAfterSave => &mut c.general.copy_after_save,
            Toggle::PlaySound => &mut c.general.play_sound,
            Toggle::CaptureOnRelease => &mut c.capture.capture_on_release,
            Toggle::Preselect => &mut c.capture.preselect_previous,
            Toggle::OpenEditor => &mut c.capture.open_editor_after_capture,
            Toggle::KeepPreviews => &mut c.capture.keep_in_deck_until_saved,
            Toggle::AlwaysShowActions => &mut c.overlay.always_show_actions,
            Toggle::SystemAudio => &mut c.recording.system_audio,
            Toggle::Keystrokes => &mut c.recording.keystrokes,
            Toggle::RecordingOpenEditor => &mut c.recording.open_editor_after,
            Toggle::RecordingSave => &mut c.recording.save,
        }
    }

    fn id(self) -> SharedString {
        SharedString::from(format!("toggle-{self:?}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigProblem {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

struct Sliders {
    quality: Entity<InspectorSlider>,
    padding: Entity<InspectorSlider>,
    radius: Entity<InspectorSlider>,
    shadow: Entity<InspectorSlider>,
    margin: Entity<InspectorSlider>,
    hide_after: Entity<InspectorSlider>,
}

pub struct SettingsWindow {
    settings: Entity<SettingsModel>,
    history: Option<Entity<History>>,
    caps: Caps,
    outputs: Vec<String>,
    page: Page,
    problem: Option<ConfigProblem>,
    save_failed: Option<String>,
    confirm_restore: Option<Page>,
    sliders: Sliders,
    file_name: Entity<InputState>,
    shortcuts: shortcuts::Shortcuts,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SettingsWindowEvent> for SettingsWindow {}

#[allow(clippy::too_many_arguments)]
fn slider(
    label: String,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    format: ValueFormat,
    edit: fn(&mut Config, f64),
    window: &mut Window,
    cx: &mut Context<SettingsWindow>,
    subscriptions: &mut Vec<Subscription>,
) -> Entity<InspectorSlider> {
    let entity = cx.new(|cx| InspectorSlider::new(label, value, range, format, window, cx));
    subscriptions.push(
        cx.subscribe(&entity, move |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Changed(v) = event {
                let v = *v;
                this.edit(move |c| edit(c, v), cx);
            }
        }),
    );
    entity
}

impl SettingsWindow {
    pub fn new(deps: SettingsDeps, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let config = deps.settings.read(cx).config().clone();
        let problem = match deps.settings.read(cx).load_error() {
            Some(es_app::errors::Error::Parse { line, col, message }) => {
                Some(ConfigProblem { line, col, message })
            }
            _ => None,
        };
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&deps.settings, window, Self::on_settings));
        let percent = ValueFormat::percent;
        let sliders = Sliders {
            quality: slider(
                fl!("general-format-quality"),
                config.general.export_quality,
                0.1..=1.0,
                percent().step(0.05),
                |c, v| c.general.export_quality = v,
                window,
                cx,
                &mut subs,
            ),
            padding: slider(
                fl!("general-look-padding"),
                config.default_look.padding,
                0.0..=0.45,
                percent(),
                |c, v| c.default_look.padding = v,
                window,
                cx,
                &mut subs,
            ),
            radius: slider(
                fl!("general-look-corner-radius"),
                config.default_look.corner_radius,
                0.0..=0.12,
                percent().digits(1),
                |c, v| c.default_look.corner_radius = v,
                window,
                cx,
                &mut subs,
            ),
            shadow: slider(
                fl!("general-look-shadow"),
                config.default_look.shadow,
                0.0..=1.0,
                percent(),
                |c, v| c.default_look.shadow = v,
                window,
                cx,
                &mut subs,
            ),
            margin: slider(
                fl!("overlay-edge-margin"),
                f64::from(config.overlay.edge_margin),
                0.0..=48.0,
                ValueFormat::points(),
                |c, v| c.overlay.edge_margin = v.round() as u32,
                window,
                cx,
                &mut subs,
            ),
            hide_after: slider(
                fl!("overlay-hide-after"),
                f64::from(config.overlay.dismiss_delay),
                2.0..=16.0,
                ValueFormat::seconds()
                    .digits(0)
                    .step(1.)
                    .named(16., fl!("overlay-hide-after-never")),
                |c, v| c.overlay.dismiss_delay = v.round() as u32,
                window,
                cx,
                &mut subs,
            ),
        };
        let template = config.general.file_name_template.clone();
        let file_name = cx.new(|cx| InputState::new(window, cx).default_value(template));
        subs.push(
            cx.subscribe(&file_name, |this, input, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    let value = input.read(cx).value().to_string();
                    this.edit(move |c| c.general.file_name_template = value, cx);
                }
            }),
        );
        let shortcuts = shortcuts::Shortcuts::new(&config, deps.binds, deps.clipboard, window, cx);
        Self {
            settings: deps.settings,
            history: deps.history,
            caps: deps.caps,
            outputs: deps.outputs,
            page: Page::Home,
            problem,
            save_failed: None,
            confirm_restore: None,
            sliders,
            file_name,
            shortcuts,
            _subscriptions: subs,
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.confirm_restore = None;
        cx.notify();
    }

    pub fn problem(&self) -> Option<&ConfigProblem> {
        self.problem.as_ref()
    }

    pub fn config(&self, cx: &App) -> Arc<Config> {
        self.settings.read(cx).config().clone()
    }

    pub fn set_toggle(&mut self, toggle: Toggle, on: bool, cx: &mut Context<Self>) {
        self.edit(move |c| *toggle.field(c) = on, cx);
    }

    fn edit(&mut self, f: impl FnOnce(&mut Config), cx: &mut Context<Self>) {
        self.settings.update(cx, |m, cx| m.update(f, cx));
    }

    fn on_settings(
        &mut self,
        _: &Entity<SettingsModel>,
        event: &SettingsEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            SettingsEvent::Changed(_) => {
                self.problem = None;
                let c = self.config(cx);
                let s = &self.sliders;
                for (entity, value) in [
                    (&s.quality, c.general.export_quality),
                    (&s.padding, c.default_look.padding),
                    (&s.radius, c.default_look.corner_radius),
                    (&s.shadow, c.default_look.shadow),
                    (&s.margin, f64::from(c.overlay.edge_margin)),
                    (&s.hide_after, f64::from(c.overlay.dismiss_delay)),
                ] {
                    if entity.read(cx).value() != value {
                        entity.update(cx, |s, cx| s.set_value(value, window, cx));
                    }
                }
            }
            SettingsEvent::Invalid { line, col, message } => {
                self.problem = Some(ConfigProblem {
                    line: *line,
                    col: *col,
                    message: message.clone(),
                });
            }
            SettingsEvent::SaveFailed(error) => self.save_failed = Some(error.clone()),
        }
        cx.notify();
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx);
        let mut list = v_flex()
            .id("settings-sidebar")
            .role(Role::TabList)
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .pt(px(SIDEBAR_TOP))
            .px(px(10.))
            .bg(t.inspector)
            .border_r_1()
            .border_color(t.border);
        for page in Page::ALL {
            let heading = match page {
                Page::General => Some(fl!("settings-nav-group-settings")),
                Page::About => Some(fl!("settings-nav-group-app")),
                _ => None,
            };
            if let Some(heading) = heading {
                list = list.child(
                    div()
                        .h(px(SIDEBAR_GROUP))
                        .pt(px(12.))
                        .px(px(8.))
                        .text_size(px(11.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.text_tertiary)
                        .child(heading),
                );
            }
            let selected = page == self.page;
            let title = page.title();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("nav-{page:?}")))
                    .accessible(Role::Tab, title.clone())
                    .aria_selected(selected)
                    .h(px(SIDEBAR_ROW))
                    .px(px(8.))
                    .gap(px(8.))
                    .rounded(px(6.))
                    .text_size(px(widgets::ROW_TEXT))
                    .text_color(t.text)
                    .when_else(
                        selected,
                        |d| d.bg(t.accent_tint),
                        |d| d.hover(|d| d.bg(t.hover_fill)),
                    )
                    .child(
                        Icon::new(page.icon())
                            .size(px(15.))
                            .text_color(if selected { t.accent } else { t.text_secondary }),
                    )
                    .child(title)
                    .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx))),
            );
        }
        list
    }

    fn banners(&self, cx: &App) -> Vec<gpui::AnyElement> {
        let mut out = Vec::new();
        if let Some(p) = &self.problem {
            out.push(widgets::alert(
                fl!("settings-config-error-title").into(),
                fl!(
                    "settings-config-error",
                    line = p.line,
                    col = p.col,
                    message = p.message.clone()
                )
                .into(),
                cx,
            ));
        }
        if let Some(error) = &self.save_failed {
            out.push(widgets::alert(
                fl!("settings-save-failed", error = error.clone()).into(),
                SharedString::default(),
                cx,
            ));
        }
        out
    }
}

trait WhenElse: Sized {
    fn when_else(
        self,
        cond: bool,
        a: impl FnOnce(Self) -> Self,
        b: impl FnOnce(Self) -> Self,
    ) -> Self {
        if cond { a(self) } else { b(self) }
    }
}

impl<T> WhenElse for T {}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children = self.banners(cx);
        children.extend(match self.page {
            Page::Home => self.render_home(cx),
            Page::General => self.render_general(cx),
            Page::Capture => self.render_capture(cx),
            Page::Overlay => self.render_overlay(cx),
            Page::Recording => self.render_recording(cx),
            Page::Shortcuts => self.render_shortcuts(window, cx),
            Page::About => self.render_about(cx),
        });
        let t = tokens(cx);
        h_flex()
            .size_full()
            .font_family(es_ui::theme::UI_FONT)
            .bg(widgets::page_fill(t))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .id(SharedString::from(format!("page-{:?}", self.page)))
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .child(widgets::page(self.page.title().into(), children, cx)),
            )
    }
}

pub fn open(deps: SettingsDeps, cx: &mut App) -> gpui::Result<WindowHandle<Root>> {
    let (w, h) = WINDOW_SIZE;
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(w), px(h)), cx)),
        window_min_size: Some(size(px(MIN_SIZE.0), px(MIN_SIZE.1))),
        app_id: Some("dev.easyshot.settings".into()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(fl!("settings-window-title").into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| SettingsWindow::new(deps, window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
}
