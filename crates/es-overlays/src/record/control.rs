use super::session::{Phase, Stopwatch, format_elapsed};
use super::{CELL, digits};
use crate::capture::strings::fl;
use es_geom::Size;
use es_protocol::Route;
use es_record::Clock;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::icons::IconName;
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    AnyElement, Context, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Task, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::Icon;
use std::sync::Arc;
use std::time::Duration;

pub const BAR: Size = Size::new(264.0, 38.0);
pub const CONFIRM: Size = Size::new(340.0, 124.0);
const STOP_WIDTH: f32 = 132.0;
const ICON_WIDTH: f32 = 42.0;
const RADIUS: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Restart,
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlEvent {
    Route(Route),
    /// `size()` changed: the surface follows it and is placed again.
    Resized,
}

pub struct ControlBar {
    phase: Phase,
    watch: Stopwatch,
    confirm: Option<Confirm>,
    _tick: Task<()>,
}

impl EventEmitter<ControlEvent> for ControlBar {}

impl ControlBar {
    pub fn new(clock: Arc<dyn Clock>, cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        ControlBar {
            phase: Phase::Starting,
            watch: Stopwatch::new(clock),
            confirm: None,
            _tick: tick,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn elapsed(&self) -> Duration {
        self.watch.elapsed()
    }

    pub fn confirm(&self) -> Option<Confirm> {
        self.confirm
    }

    pub fn size(&self) -> Size {
        if self.confirm.is_some() { CONFIRM } else { BAR }
    }

    /// The recorder reported its first frame; a start over comes back here too.
    pub fn started(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Recording;
        self.watch.start();
        cx.notify();
    }

    pub fn set_paused(&mut self, paused: bool, cx: &mut Context<Self>) {
        match (self.phase, paused) {
            (Phase::Recording, true) => {
                self.watch.pause();
                self.phase = Phase::Paused;
            }
            (Phase::Paused, false) => {
                self.watch.resume();
                self.phase = Phase::Recording;
            }
            _ => return,
        }
        cx.notify();
    }

    pub fn finishing(&mut self, cx: &mut Context<Self>) {
        self.watch.pause();
        self.phase = Phase::Finishing;
        self.set_confirm(None, cx);
    }

    fn set_confirm(&mut self, confirm: Option<Confirm>, cx: &mut Context<Self>) {
        if self.confirm != confirm {
            self.confirm = confirm;
            cx.emit(ControlEvent::Resized);
        }
        cx.notify();
    }

    fn act(&mut self, route: Route, cx: &mut Context<Self>) {
        if self.phase.busy() {
            return;
        }
        match route {
            Route::RecordRestart => self.set_confirm(Some(Confirm::Restart), cx),
            Route::RecordDiscard => self.set_confirm(Some(Confirm::Discard), cx),
            route => cx.emit(ControlEvent::Route(route)),
        }
    }

    fn confirmed(&mut self, confirm: Confirm, cx: &mut Context<Self>) {
        self.set_confirm(None, cx);
        cx.emit(ControlEvent::Route(match confirm {
            Confirm::Restart => Route::RecordRestart,
            Confirm::Discard => Route::RecordDiscard,
        }));
    }
}

fn stop_glyph(red: Hsla) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded_full()
        .border_2()
        .border_color(red)
        .child(div().size(px(6.)).rounded(px(1.5)).bg(red))
}

fn separator(colour: Hsla) -> impl IntoElement {
    div().w(px(1.)).h_full().bg(colour)
}

impl ControlBar {
    fn segment(
        &self,
        id: &'static str,
        icon: IconName,
        label: String,
        route: Route,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = tokens(cx);
        let busy = self.phase.busy();
        div()
            .id(id)
            .accessible(Role::Button, label)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(ICON_WIDTH))
            .h_full()
            .text_color(if busy { t.text_tertiary } else { t.text })
            .when(!busy, |d| {
                d.hover(|s| s.bg(t.hover_fill))
                    .active(|s| s.opacity(0.55))
                    .on_click(cx.listener(move |bar, _, _, cx| bar.act(route.clone(), cx)))
            })
            .child(Icon::new(icon).size(px(17.)))
            .into_any_element()
    }

    fn render_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let busy = self.phase.busy();
        let time = format_elapsed(self.elapsed());
        let word = match self.phase {
            Phase::Starting => fl!("rec-bar-preparing"),
            Phase::Recording => fl!("rec-bar-stop"),
            Phase::Paused => fl!("rec-bar-paused"),
            Phase::Finishing => fl!("rec-bar-saving"),
        };
        let red = if busy {
            t.destructive.opacity(0.5)
        } else {
            t.destructive
        };
        let (pause_icon, pause_label) = if self.phase == Phase::Paused {
            (IconName::CirclePlay, fl!("rec-bar-resume-help"))
        } else {
            (IconName::CirclePause, fl!("rec-bar-pause-help"))
        };
        let stop = div()
            .id("rec-stop")
            .accessible(Role::Button, fl!("rec-bar-stop-a11y", time = time.clone()))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .w(px(STOP_WIDTH))
            .h_full()
            .text_color(red)
            .when(!busy, |d| {
                d.hover(|s| s.bg(t.hover_fill))
                    .active(|s| s.opacity(0.55))
                    .on_click(cx.listener(|bar, _, _, cx| bar.act(Route::RecordStop, cx)))
            })
            .child(stop_glyph(red))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(word),
            )
            .child(digits(&time, 13., CELL).font_weight(FontWeight::MEDIUM));
        div()
            .id("rec-bar")
            .accessible(Role::Toolbar, fl!("rec-bar-a11y"))
            .flex()
            .size_full()
            .overflow_hidden()
            .rounded(px(RADIUS))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .child(stop)
            .child(separator(t.separator))
            .child(self.segment("rec-pause", pause_icon, pause_label, Route::RecordPause, cx))
            .child(separator(t.separator))
            .child(self.segment(
                "rec-restart",
                IconName::RotateCcw,
                fl!("rec-bar-start-over-help"),
                Route::RecordRestart,
                cx,
            ))
            .child(separator(t.separator))
            .child(self.segment(
                "rec-discard",
                IconName::Trash,
                fl!("rec-bar-discard-help"),
                Route::RecordDiscard,
                cx,
            ))
            .into_any_element()
    }

    fn render_confirm(&mut self, confirm: Confirm, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let (title, message, action) = match confirm {
            Confirm::Restart => (
                fl!("rec-confirm-restart-title"),
                fl!("rec-confirm-restart-message"),
                fl!("rec-confirm-restart"),
            ),
            Confirm::Discard => (
                fl!("rec-confirm-discard-title"),
                fl!("rec-confirm-discard-message"),
                fl!("rec-confirm-discard"),
            ),
        };
        let button = |id: &'static str, text: String, fill: Hsla, ink: Hsla| {
            div()
                .id(id)
                .accessible(Role::Button, text.clone())
                .flex()
                .items_center()
                .justify_center()
                .w(px(88.))
                .h(px(26.))
                .rounded(px(7.))
                .bg(fill)
                .text_color(ink)
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .hover(move |s| s.bg(fill.opacity(fill.a * 0.85)))
                .child(text)
        };
        div()
            .id("rec-confirm")
            .accessible(Role::AlertDialog, title.clone())
            .flex()
            .flex_col()
            .justify_between()
            .size_full()
            .p(px(14.))
            .rounded(px(RADIUS))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.text_secondary)
                            .child(message),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button(
                            "rec-confirm-cancel",
                            fl!("rec-confirm-cancel"),
                            t.fill,
                            t.text,
                        )
                        .on_click(cx.listener(|bar, _, _, cx| bar.set_confirm(None, cx))),
                    )
                    .child(
                        button("rec-confirm-ok", action, t.destructive, t.on_accent)
                            .on_click(cx.listener(move |bar, _, _, cx| bar.confirmed(confirm, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ControlBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.confirm {
            Some(confirm) => self.render_confirm(confirm, cx),
            None => self.render_bar(cx),
        }
    }
}
