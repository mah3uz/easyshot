use super::options::{OptionsEvent, RecordingOptions, RecordingRequest};
use crate::capture::strings::fl;
use es_geom::Size;
use es_model::platform::OutputInfo;
use es_protocol::Route;
use es_record::AudioDevice;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::icons::IconName;
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
};
use gpui_component::{Icon, tooltip::Tooltip};

pub const BAR: Size = Size::new(676.0, 64.0);
const POPUP_GAP: f64 = 8.0;
const TIMER_MENU: Size = Size::new(220.0, 10.0 * 24.0 + 8.0);
const SCREENSHOT_TIMERS: [u32; 4] = [0, 3, 5, 10];
const RECORDING_TIMERS: [u32; 4] = [0, 1, 3, 5];

#[derive(Debug, Clone)]
pub enum BarEvent {
    Route(Route),
    Close,
    ScreenshotTimer(u32),
    RecordingTimer(u32),
    Record(Box<RecordingRequest>),
    RefreshWindows,
    /// `size()` changed.
    Resized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    Timer,
    Options,
}

/// An icon above its caption; `on` is full ink, off is dimmed, never resized.
pub(crate) fn captioned(
    id: &'static str,
    icon: IconName,
    caption: String,
    tip: String,
    label: String,
    on: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = tokens(cx);
    let ink = if on { t.text } else { t.text.opacity(0.4) };
    let hover = t.hover_fill;
    let tip: SharedString = tip.into();
    div()
        .id(id)
        .accessible(Role::Button, label)
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .w(px(72.))
        .h(px(52.))
        .rounded(px(10.))
        .text_color(ink)
        .hover(move |s| s.bg(hover))
        .active(|s| s.opacity(0.55))
        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
        .child(Icon::new(icon).size(px(22.)))
        .child(div().text_size(px(11.)).child(caption))
}

pub(crate) fn divider(colour: Hsla) -> impl IntoElement {
    div().flex_none().mx(px(5.)).w(px(1.)).h(px(24.)).bg(colour)
}

pub struct CaptureBar {
    popup: Option<Popup>,
    screenshot_timer: u32,
    options: Entity<RecordingOptions>,
    focus: FocusHandle,
    focused: bool,
    _options: Subscription,
}

impl EventEmitter<BarEvent> for CaptureBar {}

impl Focusable for CaptureBar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CaptureBar {
    pub fn new(
        outputs: Vec<OutputInfo>,
        devices: Vec<AudioDevice>,
        cx: &mut Context<Self>,
    ) -> Self {
        let options = cx.new(|_| RecordingOptions::new(outputs, devices));
        let sub = cx.subscribe(&options, |bar, _, event: &OptionsEvent, cx| match event {
            OptionsEvent::Start(request) => {
                bar.set_popup(None, cx);
                cx.emit(BarEvent::Record(request.clone()));
            }
            OptionsEvent::RefreshWindows => cx.emit(BarEvent::RefreshWindows),
            OptionsEvent::Delay(secs) => cx.emit(BarEvent::RecordingTimer(*secs)),
            OptionsEvent::Resized => {
                cx.emit(BarEvent::Resized);
                cx.notify();
            }
        });
        CaptureBar {
            popup: None,
            screenshot_timer: 0,
            options,
            focus: cx.focus_handle(),
            focused: false,
            _options: sub,
        }
    }

    pub fn options(&self) -> &Entity<RecordingOptions> {
        &self.options
    }

    pub fn popup(&self) -> Option<Popup> {
        self.popup
    }

    pub fn screenshot_timer(&self) -> u32 {
        self.screenshot_timer
    }

    pub fn size(&self, cx: &App) -> Size {
        let popup = match self.popup {
            None => return BAR,
            Some(Popup::Timer) => TIMER_MENU,
            Some(Popup::Options) => self.options.read(cx).size(),
        };
        Size::new(
            BAR.width.max(popup.width),
            BAR.height + POPUP_GAP + popup.height,
        )
    }

    fn set_popup(&mut self, popup: Option<Popup>, cx: &mut Context<Self>) {
        if self.popup != popup {
            self.popup = popup;
            cx.emit(BarEvent::Resized);
        }
        cx.notify();
    }

    fn toggle(&mut self, popup: Popup, cx: &mut Context<Self>) {
        self.set_popup((self.popup != Some(popup)).then_some(popup), cx);
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" {
            match self.popup {
                Some(_) => self.set_popup(None, cx),
                None => cx.emit(BarEvent::Close),
            }
        }
    }

    fn render_timer_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let delay = self.options.read(cx).delay();
        let header = |text: String| {
            div()
                .h(px(24.))
                .flex()
                .items_center()
                .px(px(8.))
                .text_size(px(11.))
                .text_color(t.text_secondary)
                .child(text)
        };
        let row = |id: (&'static str, usize), text: String, checked: bool| {
            div()
                .id(id)
                .accessible(Role::MenuItem, text.clone())
                .toggled(checked)
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(24.))
                .px(px(8.))
                .rounded(px(5.))
                .text_size(px(12.))
                .hover(|s| s.bg(t.hover_fill))
                .child(div().w(px(14.)).when(checked, |d| {
                    d.child(Icon::new(IconName::Check).size(px(12.)))
                }))
                .child(text)
        };
        let mut menu = div()
            .id("bar-timer-menu")
            .accessible(Role::Menu, fl!("bar-timer-a11y"))
            .flex()
            .flex_col()
            .p(px(4.))
            .size_full()
            .rounded(px(10.))
            .bg(t.dropdown)
            .border_1()
            .border_color(t.border)
            .text_color(t.text)
            .child(header(fl!("bar-timer-section-screenshot")));
        for (i, secs) in SCREENSHOT_TIMERS.into_iter().enumerate() {
            let text = match secs {
                0 => fl!("bar-timer-screenshot-off"),
                s => fl!("bar-timer-screenshot-seconds", seconds = s),
            };
            menu = menu.child(
                row(("bar-timer-shot", i), text, secs == self.screenshot_timer).on_click(
                    cx.listener(move |bar, _, _, cx| {
                        bar.screenshot_timer = secs;
                        bar.set_popup(None, cx);
                        cx.emit(BarEvent::ScreenshotTimer(secs));
                    }),
                ),
            );
        }
        menu = menu.child(header(fl!("bar-timer-section-recording")));
        for (i, secs) in RECORDING_TIMERS.into_iter().enumerate() {
            let text = match secs {
                0 => fl!("bar-timer-recording-none"),
                s => fl!("bar-timer-recording-seconds", count = s),
            };
            menu = menu.child(row(("bar-timer-rec", i), text, secs == delay).on_click(
                cx.listener(move |bar, _, _, cx| {
                    bar.options.update(cx, |o, cx| o.set_delay(secs, cx));
                    bar.set_popup(None, cx);
                    cx.emit(BarEvent::RecordingTimer(secs));
                }),
            ));
        }
        menu.into_any_element()
    }
}

const ITEMS: [(&str, IconName, Route); 6] = [
    ("bar-area", IconName::Scan, Route::CaptureRegion),
    (
        "bar-fullscreen",
        IconName::Monitor,
        Route::CaptureFullscreen,
    ),
    ("bar-window", IconName::AppWindow, Route::CaptureWindow),
    ("bar-scroll", IconName::UnfoldVertical, Route::CaptureScroll),
    ("bar-ocr", IconName::ALargeSmall, Route::Ocr),
    ("bar-color", IconName::Pipette, Route::ColorPicker),
];

fn words(id: &str) -> (String, String) {
    match id {
        "bar-area" => (fl!("bar-caption-area"), fl!("bar-title-area")),
        "bar-fullscreen" => (fl!("bar-caption-screen"), fl!("bar-title-screen")),
        "bar-window" => (fl!("bar-caption-window"), fl!("bar-title-window")),
        "bar-scroll" => (fl!("bar-caption-scroll"), fl!("bar-title-scrolling")),
        "bar-ocr" => (fl!("bar-caption-text"), fl!("bar-title-text")),
        _ => (fl!("bar-caption-color"), fl!("bar-title-color")),
    }
}

impl Render for CaptureBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let t = tokens(cx).clone();
        let mut bar = div()
            .id("capture-bar")
            .accessible(Role::Toolbar, fl!("bar-a11y"))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .px(px(6.))
            .w(px(BAR.width as f32))
            .h(px(BAR.height as f32))
            .rounded(px(16.))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border);
        for (id, icon, route) in ITEMS {
            let (caption, title) = words(id);
            bar = bar.child(
                captioned(id, icon, caption, title.clone(), title, true, cx).on_click(
                    cx.listener(move |_, _, _, cx| cx.emit(BarEvent::Route(route.clone()))),
                ),
            );
        }
        let close_tip: SharedString = fl!("bar-close-help").into();
        bar = bar
            .child(divider(t.separator))
            .child(
                captioned(
                    "bar-timer",
                    IconName::Timer,
                    fl!("bar-caption-timer"),
                    fl!("bar-timer-a11y"),
                    fl!("bar-timer-a11y"),
                    true,
                    cx,
                )
                .toggled(self.popup == Some(Popup::Timer))
                .on_click(cx.listener(|bar, _, _, cx| bar.toggle(Popup::Timer, cx))),
            )
            .child(
                captioned(
                    "bar-recording",
                    IconName::Video,
                    fl!("bar-caption-recording"),
                    fl!("bar-recording-help"),
                    fl!("bar-recording-help"),
                    true,
                    cx,
                )
                .toggled(self.popup == Some(Popup::Options))
                .on_click(cx.listener(|bar, _, _, cx| bar.toggle(Popup::Options, cx))),
            )
            .child(
                div()
                    .id("bar-close")
                    .accessible(Role::Button, fl!("bar-close-a11y"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded(px(10.))
                    .text_color(t.text)
                    .hover(|s| s.bg(t.hover_fill))
                    .active(|s| s.opacity(0.55))
                    .tooltip(move |window, cx| Tooltip::new(close_tip.clone()).build(window, cx))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(BarEvent::Close)))
                    .child(Icon::new(IconName::X).size(px(17.))),
            );
        let popup = self.popup.map(|p| match p {
            Popup::Timer => div()
                .w(px(TIMER_MENU.width as f32))
                .h(px(TIMER_MENU.height as f32))
                .child(self.render_timer_menu(cx)),
            Popup::Options => {
                let size = self.options.read(cx).size();
                div()
                    .w(px(size.width as f32))
                    .h(px(size.height as f32))
                    .child(self.options.clone())
            }
        });
        div()
            .id("capture-bar-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|bar, e: &KeyDownEvent, _, cx| bar.key(e, cx)))
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .gap(px(POPUP_GAP as f32))
            .font_family(UI_FONT)
            .text_color(t.text)
            .when_some(popup, |d, p| d.child(p))
            .child(bar)
    }
}
