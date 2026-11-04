use super::bar::{captioned, divider};
use super::groups::window_groups;
use crate::capture::strings::fl;
use es_geom::{Rect, Size};
use es_model::platform::{OutputId, OutputInfo, WindowInfo};
use es_record::{AudioDevice, AudioKind, RecordSource};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::components::icon_button::IconButton;
use es_ui::icons::IconName;
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    AnyElement, Context, EventEmitter, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::Icon;

pub const WIDTH: f64 = 460.0;
const PAD: f64 = 20.0;
const GAP: f64 = 16.0;
const ROW: f64 = 26.0;
const MAX_DELAY: u32 = 60;
const BASE_HEIGHT: f64 = PAD + 18.0 + 2.0 + 15.0 + GAP + 52.0 + GAP + 28.0 + PAD;

#[derive(Debug, Clone)]
pub enum RecordingTarget {
    /// The region selector, without window mode, picks it next.
    Area,
    Display(OutputId),
    Window(WindowInfo),
}

#[derive(Debug, Clone)]
pub struct RecordingRequest {
    pub target: RecordingTarget,
    /// A PulseAudio source name.
    pub mic: Option<String>,
    pub system_audio: bool,
    pub delay_secs: u32,
}

impl RecordingRequest {
    /// `area` is the selector's answer for an area target, relative to its output.
    pub fn source(&self, area: Option<(OutputId, Rect)>) -> Option<RecordSource> {
        match &self.target {
            RecordingTarget::Area => {
                area.map(|(output, logical)| RecordSource::Region { output, logical })
            }
            RecordingTarget::Display(id) => Some(RecordSource::Output(id.clone())),
            RecordingTarget::Window(w) => Some(RecordSource::Window(w.clone())),
        }
    }
}

/// What the panel starts with; changing it on the panel affects only that recording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordingDefaults {
    pub mic: Option<String>,
    pub system_audio: bool,
    pub delay_secs: u32,
}

#[derive(Debug, Clone)]
pub enum OptionsEvent {
    Start(Box<RecordingRequest>),
    RefreshWindows,
    Delay(u32),
    /// `height()` changed.
    Resized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Display,
    Window,
    Mic,
}

#[derive(Clone)]
enum Pick {
    Display(OutputId),
    Window(usize),
    Refresh,
    Mic(Option<String>),
}

pub struct RecordingOptions {
    outputs: Vec<OutputInfo>,
    windows: Vec<WindowInfo>,
    mics: Vec<AudioDevice>,
    mic: Option<String>,
    system_audio: bool,
    delay: u32,
    menu: Option<Menu>,
}

impl EventEmitter<OptionsEvent> for RecordingOptions {}

impl RecordingOptions {
    pub fn new(outputs: Vec<OutputInfo>, devices: Vec<AudioDevice>) -> Self {
        RecordingOptions {
            outputs,
            windows: Vec::new(),
            mics: devices
                .into_iter()
                .filter(|d| d.kind == AudioKind::Microphone)
                .collect(),
            mic: None,
            system_audio: false,
            delay: 0,
            menu: None,
        }
    }

    pub fn set_windows(&mut self, windows: Vec<WindowInfo>, cx: &mut Context<Self>) {
        self.windows = windows;
        self.resized(cx);
    }

    pub fn set_defaults(&mut self, defaults: RecordingDefaults, cx: &mut Context<Self>) {
        self.mic = defaults.mic;
        self.system_audio = defaults.system_audio;
        self.delay = defaults.delay_secs.min(MAX_DELAY);
        cx.notify();
    }

    pub fn set_delay(&mut self, secs: u32, cx: &mut Context<Self>) {
        self.delay = secs.min(MAX_DELAY);
        cx.notify();
    }

    pub fn delay(&self) -> u32 {
        self.delay
    }

    pub fn menu(&self) -> Option<Menu> {
        self.menu
    }

    pub fn request(&self, target: RecordingTarget) -> RecordingRequest {
        RecordingRequest {
            target,
            mic: self.mic.clone(),
            system_audio: self.system_audio,
            delay_secs: self.delay,
        }
    }

    pub fn size(&self) -> Size {
        let rows = self.menu.map_or(0, |m| self.rows(m).len());
        let menu = if rows == 0 {
            0.0
        } else {
            rows as f64 * ROW + 8.0 + GAP
        };
        Size::new(WIDTH, BASE_HEIGHT + menu)
    }

    fn resized(&mut self, cx: &mut Context<Self>) {
        cx.emit(OptionsEvent::Resized);
        cx.notify();
    }

    fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = (self.menu != Some(menu)).then_some(menu);
        self.resized(cx);
    }

    fn start(&mut self, target: RecordingTarget, cx: &mut Context<Self>) {
        let request = self.request(target);
        self.menu = None;
        self.resized(cx);
        cx.emit(OptionsEvent::Start(Box::new(request)));
    }

    fn display(&mut self, cx: &mut Context<Self>) {
        match self.outputs.as_slice() {
            [only] => self.start(RecordingTarget::Display(only.id.clone()), cx),
            _ => self.toggle_menu(Menu::Display, cx),
        }
    }

    fn pick(&mut self, pick: Pick, cx: &mut Context<Self>) {
        match pick {
            Pick::Display(id) => self.start(RecordingTarget::Display(id), cx),
            Pick::Window(i) => {
                if let Some(w) = self.windows.get(i).cloned() {
                    self.start(RecordingTarget::Window(w), cx);
                }
            }
            Pick::Refresh => cx.emit(OptionsEvent::RefreshWindows),
            Pick::Mic(name) => {
                self.mic = name;
                self.menu = None;
                self.resized(cx);
            }
        }
    }

    fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        let secs = self.delay.saturating_add_signed(by).min(MAX_DELAY);
        if secs != self.delay {
            self.delay = secs;
            cx.emit(OptionsEvent::Delay(secs));
            cx.notify();
        }
    }

    fn rows(&self, menu: Menu) -> Vec<(String, bool, Option<Pick>)> {
        match menu {
            Menu::Display => self
                .outputs
                .iter()
                .enumerate()
                .map(|(n, o)| {
                    let number = n + 1;
                    let name = if o.description.is_empty() {
                        fl!("rec-display-fallback", n = number)
                    } else {
                        o.description.clone()
                    };
                    let (w, h) = o.physical;
                    let text = fl!("rec-display-item", name = name, w = w, h = h);
                    (text, false, Some(Pick::Display(o.id.clone())))
                })
                .collect(),
            Menu::Window => {
                let mut rows = Vec::new();
                for group in window_groups(&self.windows) {
                    if let Some(name) = group.workspace {
                        let header = if group.shown {
                            fl!("rec-windows-workspace-shown", name = name)
                        } else {
                            fl!("rec-windows-workspace", name = name)
                        };
                        rows.push((header, false, None));
                    }
                    for i in group.windows {
                        let w = &self.windows[i];
                        let text = if w.title.is_empty() {
                            fl!("rec-window-item-app-only", app = w.app_id.clone())
                        } else {
                            fl!(
                                "rec-window-item",
                                app = w.app_id.clone(),
                                title = w.title.clone()
                            )
                        };
                        rows.push((text, false, Some(Pick::Window(i))));
                    }
                }
                if rows.is_empty() {
                    rows.push((fl!("rec-windows-empty"), false, None));
                }
                rows.push((fl!("rec-windows-refresh"), false, Some(Pick::Refresh)));
                rows
            }
            Menu::Mic => std::iter::once((
                fl!("rec-mic-off"),
                self.mic.is_none(),
                Some(Pick::Mic(None)),
            ))
            .chain(self.mics.iter().map(|d| {
                let on = self.mic.as_deref().is_some_and(|m| same_mic(&d.name, m));
                (
                    d.description.clone(),
                    on,
                    Some(Pick::Mic(Some(d.name.clone()))),
                )
            }))
            .collect(),
        }
    }

    fn render_menu(&self, menu: Menu, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let label = match menu {
            Menu::Display => fl!("rec-a11y-display-multi"),
            Menu::Window => fl!("rec-a11y-window"),
            Menu::Mic => fl!("rec-a11y-mic-off"),
        };
        let mut list = div()
            .id("rec-menu")
            .accessible(Role::Menu, label)
            .flex()
            .flex_col()
            .p(px(4.))
            .rounded(px(8.))
            .bg(t.dropdown)
            .border_1()
            .border_color(t.border);
        for (i, (text, checked, pick)) in self.rows(menu).into_iter().enumerate() {
            let live = pick.is_some();
            list = list.child(
                div()
                    .id(("rec-menu-row", i))
                    .accessible(Role::MenuItem, text.clone())
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(ROW as f32))
                    .px(px(8.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .text_color(if live { t.text } else { t.text_tertiary })
                    .child(div().w(px(14.)).when(checked, |d| {
                        d.child(Icon::new(IconName::Check).size(px(12.)))
                    }))
                    .child(div().flex_1().truncate().child(text))
                    .when_some(pick, |d, pick| {
                        d.hover(|s| s.bg(t.hover_fill))
                            .on_click(cx.listener(move |o, _, _, cx| o.pick(pick.clone(), cx)))
                    }),
            );
        }
        list.into_any_element()
    }
}

// Settings stores the default input as "default"; the device list names it "default_input".
fn same_mic(device: &str, chosen: &str) -> bool {
    device == chosen || (device == "default_input" && chosen == "default")
}

impl Render for RecordingOptions {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let multi = self.outputs.len() > 1;
        let mic_name = self.mic.as_ref().map(|name| {
            self.mics
                .iter()
                .find(|d| same_mic(&d.name, name))
                .map_or(name.clone(), |d| d.description.clone())
        });
        let (display_tip, display_a11y) = if multi {
            (fl!("rec-tip-display-multi"), fl!("rec-a11y-display-multi"))
        } else {
            (
                fl!("rec-tip-display-single"),
                fl!("rec-a11y-display-single"),
            )
        };
        let (mic_icon, mic_tip, mic_a11y) = match &mic_name {
            Some(name) => (
                IconName::Mic,
                fl!("rec-tip-mic-on"),
                fl!("rec-a11y-mic-on", name = name.clone()),
            ),
            None => (
                IconName::MicOff,
                fl!("rec-tip-mic-off"),
                fl!("rec-a11y-mic-off"),
            ),
        };
        let (audio_icon, audio_tip, audio_a11y) = if self.system_audio {
            (
                IconName::Volume2,
                fl!("rec-tip-audio-on"),
                fl!("rec-a11y-audio-on"),
            )
        } else {
            (
                IconName::VolumeOff,
                fl!("rec-tip-audio-off"),
                fl!("rec-a11y-audio-off"),
            )
        };
        let sources = div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(
                captioned(
                    "rec-area",
                    IconName::SquareDashed,
                    fl!("rec-caption-area"),
                    fl!("rec-tip-area"),
                    fl!("rec-a11y-area"),
                    true,
                    cx,
                )
                .on_click(cx.listener(|o, _, _, cx| o.start(RecordingTarget::Area, cx))),
            )
            .child(
                captioned(
                    "rec-display",
                    IconName::PanelTop,
                    fl!("rec-caption-display"),
                    display_tip,
                    display_a11y,
                    true,
                    cx,
                )
                .on_click(cx.listener(|o, _, _, cx| o.display(cx))),
            )
            .child(
                captioned(
                    "rec-window",
                    IconName::AppWindow,
                    fl!("rec-caption-window"),
                    fl!("rec-tip-window"),
                    fl!("rec-a11y-window"),
                    true,
                    cx,
                )
                .on_click(cx.listener(|o, _, _, cx| o.toggle_menu(Menu::Window, cx))),
            )
            .child(divider(t.separator))
            .child(
                captioned(
                    "rec-mic",
                    mic_icon,
                    fl!("rec-caption-mic"),
                    mic_tip,
                    mic_a11y,
                    mic_name.is_some(),
                    cx,
                )
                .toggled(mic_name.is_some())
                .on_click(cx.listener(|o, _, _, cx| o.toggle_menu(Menu::Mic, cx))),
            )
            .child(
                captioned(
                    "rec-audio",
                    audio_icon,
                    fl!("rec-caption-audio"),
                    audio_tip,
                    audio_a11y,
                    self.system_audio,
                    cx,
                )
                .toggled(self.system_audio)
                .on_click(cx.listener(|o, _, _, cx| {
                    o.system_audio = !o.system_audio;
                    cx.notify();
                })),
            );
        let delay = fl!("rec-delay-value", seconds = self.delay);
        let stepper = div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(28.))
            .child(div().text_size(px(12.)).child(fl!("rec-delay-label")))
            .child(
                div()
                    .id("rec-delay")
                    .accessible(Role::Group, fl!("rec-delay-a11y", seconds = self.delay))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        IconButton::new("rec-delay-less", IconName::Minus, fl!("rec-delay-less"))
                            .size(px(24.), px(12.))
                            .disabled(self.delay == 0)
                            .on_click(cx.listener(|o, _, _, cx| o.step(-1, cx))),
                    )
                    .child(
                        div()
                            .w(px(40.))
                            .flex()
                            .justify_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(delay),
                    )
                    .child(
                        IconButton::new("rec-delay-more", IconName::Plus, fl!("rec-delay-more"))
                            .size(px(24.), px(12.))
                            .disabled(self.delay >= MAX_DELAY)
                            .on_click(cx.listener(|o, _, _, cx| o.step(1, cx))),
                    ),
            );
        div()
            .id("rec-options")
            .accessible(Role::Dialog, fl!("rec-options-title"))
            .flex()
            .flex_col()
            .gap(px(GAP as f32))
            .size_full()
            .p(px(PAD as f32))
            .rounded(px(16.))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .h(px(18.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(fl!("rec-options-title")),
                    )
                    .child(
                        div()
                            .h(px(15.))
                            .text_size(px(11.))
                            .text_color(t.text_secondary)
                            .child(fl!("rec-options-subtitle")),
                    ),
            )
            .child(sources)
            .when_some(self.menu, |d, menu| d.child(self.render_menu(menu, cx)))
            .child(stepper)
    }
}
