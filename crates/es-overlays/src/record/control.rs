use super::placement::clamp;
use super::session::{Phase, Stopwatch, format_elapsed};
use super::{CELL, digits};
use crate::capture::strings::fl;
use crate::pin::{PointerGlobal, dragged};
use es_geom::{Point, Rect, Size};
use es_protocol::Route;
use es_record::{AudioSource, Clock};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::components::Tip;
use es_ui::icons::IconName;
use es_ui::motion::{BAR_SWAP, Clocked, HANDOFF_IN, HANDOFF_SCALE, progress, since_change};
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    AnyElement, Context, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Task, Window, canvas, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::Icon;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

mod confirm;

pub const BAR: Size = Size::new(282.0, 38.0);
pub const CONFIRM: Size = Size::new(340.0, 124.0);
/// Clear space above the bar, so a tooltip opens over the desktop instead of over the controls.
pub const TIP_ROOM: f64 = 30.0;
const STOP_WIDTH: f32 = 132.0;
const ICON_WIDTH: f32 = 42.0;
const GRIP_WIDTH: f32 = 18.0;
const RADIUS: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Restart,
    Discard,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ControlEvent {
    Route(Route),
    /// `size()` changed: the surface follows it and is placed again.
    Resized,
    /// Dragged to `origin()`.
    Moved,
    /// A drag ended at `origin()`.
    Dropped,
    Mute(AudioSource, bool),
}

#[derive(Clone, Copy)]
struct Press {
    local: Point,
    global: Option<Point>,
    origin: Point,
}

pub struct ControlBar {
    phase: Phase,
    watch: Stopwatch,
    confirm: Option<Confirm>,
    audio: Vec<AudioSource>,
    muted: Vec<AudioSource>,
    origin: Point,
    output: Option<Rect>,
    pointer: PointerGlobal,
    press: Option<Press>,
    entry: Option<Clocked>,
    swap: Option<Clocked>,
    _tick: Task<()>,
}

impl EventEmitter<ControlEvent> for ControlBar {}

fn local(p: gpui::Point<Pixels>) -> Point {
    Point::new(f64::from(f32::from(p.x)), f64::from(f32::from(p.y)))
}

impl ControlBar {
    /// `audio` is what the recording takes sound from; each gets a mute button.
    pub fn new(clock: Arc<dyn Clock>, audio: Vec<AudioSource>, cx: &mut Context<Self>) -> Self {
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
            audio,
            muted: Vec::new(),
            origin: Point::ZERO,
            output: None,
            pointer: Rc::new(|| None),
            press: None,
            entry: Clocked::start(HANDOFF_IN, cx),
            swap: None,
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

    /// True while the bar still fades and springs in from the capture bar it replaces.
    pub fn entering(&self, cx: &gpui::App) -> bool {
        self.entry.is_some_and(|e| !e.done(cx))
    }

    fn content(&self) -> Size {
        match self.confirm {
            Some(_) => CONFIRM,
            None => {
                let mutes = self.audio.len() as f64 * (f64::from(ICON_WIDTH) + 1.0);
                Size::new(BAR.width + mutes, BAR.height)
            }
        }
    }

    /// The surface: the bar or the confirm card, with the tooltip room above.
    pub fn size(&self) -> Size {
        let c = self.content();
        Size::new(c.width, c.height + TIP_ROOM)
    }

    /// The part that takes the pointer, in surface coordinates.
    pub fn hit_area(&self) -> Rect {
        let c = self.content();
        Rect::new(0.0, TIP_ROOM, c.width, TIP_ROOM + c.height)
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    pub fn output(&self) -> Option<Rect> {
        self.output
    }

    /// Where the host put the surface, on which output, and how to ask for the pointer there.
    pub fn place(&mut self, origin: Point, output: Rect, pointer: PointerGlobal) {
        self.origin = origin;
        self.output = Some(output);
        self.pointer = pointer;
    }

    pub fn is_muted(&self, source: AudioSource) -> bool {
        self.muted.contains(&source)
    }

    pub fn set_muted(&mut self, source: AudioSource, muted: bool, cx: &mut Context<Self>) {
        self.muted.retain(|s| *s != source);
        if muted {
            self.muted.push(source);
        }
        cx.notify();
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

    /// A confirmed card stays up, inert, until the surface closes, so the bar never flashes back
    /// between the card and leaving.
    pub fn finishing(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Finishing {
            return;
        }
        self.watch.pause();
        self.phase = Phase::Finishing;
        self.set_confirm(None, cx);
    }

    /// The bottom edge stays put and the bar stays centred on where it was.
    fn set_confirm(&mut self, confirm: Option<Confirm>, cx: &mut Context<Self>) {
        if self.confirm != confirm {
            let old = self.size();
            self.confirm = confirm;
            self.swap = Clocked::start(BAR_SWAP, cx);
            let new = self.size();
            let moved = Point::new(
                self.origin.x + (old.width - new.width) / 2.0,
                self.origin.y + old.height - new.height,
            );
            self.origin = self.output.map_or(moved, |o| clamp(moved, new, o));
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

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.phase.busy() {
            self.set_confirm(None, cx);
        }
    }

    fn confirmed(&mut self, confirm: Confirm, cx: &mut Context<Self>) {
        if self.phase.busy() {
            return;
        }
        self.watch.pause();
        self.phase = Phase::Finishing;
        cx.notify();
        cx.emit(ControlEvent::Route(match confirm {
            Confirm::Restart => Route::RecordRestart,
            Confirm::Discard => Route::RecordDiscard,
        }));
    }

    fn grab(&mut self, at: gpui::Point<Pixels>) {
        self.press = Some(Press {
            local: local(at),
            global: (self.pointer)(),
            origin: self.origin,
        });
    }

    fn drag(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(press) = self.press else {
            return;
        };
        if e.pressed_button != Some(MouseButton::Left) {
            self.press = None;
            return;
        }
        // While the button is held, positions stay relative to the surface as it was at the press.
        let to = match (press.global, (self.pointer)()) {
            (Some(from), Some(to)) => dragged(press.origin, from, to),
            _ => dragged(press.origin, press.local, local(e.position)),
        };
        let to = self.output.map_or(to, |o| clamp(to, self.size(), o));
        if to != self.origin {
            self.origin = to;
            cx.emit(ControlEvent::Moved);
        }
    }

    fn release(&mut self, e: &MouseUpEvent, cx: &mut Context<Self>) {
        if e.button == MouseButton::Left && self.press.take().is_some() {
            cx.emit(ControlEvent::Dropped);
        }
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

fn tip(d: Stateful<gpui::Div>, text: String) -> Stateful<gpui::Div> {
    let text: SharedString = text.into();
    d.tip(text)
}

impl ControlBar {
    fn segment(
        &self,
        id: &'static str,
        icon: IconName,
        label: String,
        ink: Option<Hsla>,
        shown: f32,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        let t = tokens(cx);
        let busy = self.phase.busy();
        let d = div()
            .id(id)
            .accessible(Role::Button, label.clone())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(ICON_WIDTH))
            .h_full()
            .text_color(match (busy, ink) {
                (true, _) => t.text_tertiary,
                (false, Some(ink)) => ink,
                (false, None) => t.text,
            })
            .when(!busy, |d| {
                d.hover(|s| s.bg(t.hover_fill)).active(|s| s.opacity(0.55))
            })
            .child(div().opacity(shown).child(Icon::new(icon).size(px(17.))));
        tip(d, label)
    }

    fn routed(
        &self,
        id: &'static str,
        icon: IconName,
        label: String,
        route: Route,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.segment(id, icon, label, None, 1.0, cx)
            .on_click(cx.listener(move |bar, _, _, cx| bar.act(route.clone(), cx)))
            .into_any_element()
    }

    fn mute(&self, source: AudioSource, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let muted = self.is_muted(source);
        let (id, icon, label) = match (source, muted) {
            (AudioSource::Mic, false) => {
                ("rec-mute-mic", IconName::Mic, fl!("rec-bar-mute-mic-help"))
            }
            (AudioSource::Mic, true) => (
                "rec-mute-mic",
                IconName::MicOff,
                fl!("rec-bar-unmute-mic-help"),
            ),
            (AudioSource::System, false) => (
                "rec-mute-system",
                IconName::Volume2,
                fl!("rec-bar-mute-system-help"),
            ),
            (AudioSource::System, true) => (
                "rec-mute-system",
                IconName::VolumeOff,
                fl!("rec-bar-unmute-system-help"),
            ),
        };
        let red = muted.then(|| tokens(cx).destructive);
        let shown = since_change(id, muted, BAR_SWAP, window, cx);
        self.segment(id, icon, label, red, shown, cx)
            .toggled(muted)
            .on_click(cx.listener(move |bar, _, _, cx| {
                if !bar.phase.busy() {
                    cx.emit(ControlEvent::Mute(source, !bar.is_muted(source)));
                }
            }))
            .into_any_element()
    }

    fn grip(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx);
        let d = div()
            .id("rec-grip")
            .accessible(Role::Button, fl!("rec-bar-move-help"))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(GRIP_WIDTH))
            .h_full()
            .pl(px(4.))
            .text_color(t.text_tertiary)
            .cursor_grab()
            .child(Icon::new(IconName::GripVertical).size(px(13.)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|bar, e: &gpui::MouseDownEvent, _, _| bar.grab(e.position)),
            );
        tip(d, fl!("rec-bar-move-help")).into_any_element()
    }

    fn render_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        let busy = self.phase.busy();
        let time = format_elapsed(self.elapsed());
        let word = match self.phase {
            Phase::Starting => fl!("rec-bar-preparing"),
            Phase::Recording => fl!("rec-bar-stop"),
            Phase::Paused => fl!("rec-bar-paused"),
            Phase::Finishing => fl!("rec-bar-saving"),
        };
        let word_shown = since_change("rec-stop-word", self.phase, BAR_SWAP, window, cx);
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
                    .opacity(word_shown)
                    .child(word),
            )
            .child(digits(&time, 13., CELL).font_weight(FontWeight::MEDIUM));
        let mut bar = div()
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
            .child(self.grip(cx))
            .child(tip(stop, fl!("rec-bar-stop-help")));
        for source in self.audio.clone() {
            bar = bar
                .child(separator(t.separator))
                .child(self.mute(source, window, cx));
        }
        bar.child(separator(t.separator))
            .child(self.routed("rec-pause", pause_icon, pause_label, Route::RecordPause, cx))
            .child(separator(t.separator))
            .child(self.routed(
                "rec-restart",
                IconName::RotateCcw,
                fl!("rec-bar-start-over-help"),
                Route::RecordRestart,
                cx,
            ))
            .child(separator(t.separator))
            .child(self.routed(
                "rec-discard",
                IconName::Trash,
                fl!("rec-bar-discard-help"),
                Route::RecordDiscard,
                cx,
            ))
            .into_any_element()
    }
}

impl Render for ControlBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.confirm {
            Some(confirm) => self.render_confirm(confirm, cx),
            None => self.render_bar(window, cx),
        };
        let swapped = progress(self.swap, window, cx);
        let e = progress(self.entry, window, cx);
        let scale = HANDOFF_SCALE + (1.0 - HANDOFF_SCALE) * e;
        let c = self.content();
        let view = cx.entity();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_end()
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let moving = view.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, _, _, cx| {
                            moving.update(cx, |bar, cx| bar.drag(e, cx));
                        });
                        let releasing = view.clone();
                        window.on_mouse_event(move |e: &MouseUpEvent, _, _, cx| {
                            releasing.update(cx, |bar, cx| bar.release(e, cx));
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(c.width as f32))
                    .h(px(c.height as f32))
                    .child(
                        div()
                            .w(px(c.width as f32 * scale))
                            .h(px(c.height as f32 * scale))
                            .opacity(e.min(1.0) * swapped)
                            .child(body),
                    ),
            )
    }
}
